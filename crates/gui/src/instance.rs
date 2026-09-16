//! 单实例控制.
//!
//! 锁文件与数据目录绑定, 因此正式实例与 `just debug` / `just fake-dist` 的隔离实例可以并行运行.
//! 二次启动时新进程把启动参数转发给已有实例, 已有实例收到通知后显示并聚焦主窗口, 新进程自身退出.

use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::paths;

/// 参数之间的分隔符, 不会出现在正常的命令行参数里.
const ARG_SEPARATOR: char = '\u{1f}';
/// 二次启动通知的连接超时.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// 更新后重新拉起新进程时由旧进程设置的环境变量.
pub const RELAUNCH_ENV: &str = "STCJUDGE_GUI_RELAUNCH";
/// 重新拉起的实例等待旧实例释放锁的重试间隔.
const RELAUNCH_RETRY_DELAY: Duration = Duration::from_millis(200);
/// 重新拉起的实例等待旧实例释放锁的最大次数, 约 5 秒.
const RELAUNCH_MAX_ATTEMPTS: u32 = 25;

/// 主实例句柄, drop 时释放锁并删除端口文件.
pub struct InstanceGuard {
    lock: File,
    lock_path: PathBuf,
    port_path: PathBuf,
    messages: Receiver<Vec<String>>,
}

impl InstanceGuard {
    /// 二次启动转发过来的参数.
    pub fn try_recv(&self) -> Option<Vec<String>> {
        match self.messages.try_recv() {
            Ok(args) => Some(args),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.port_path);
        let _ = self.lock.unlock();
        debug!(lock = %self.lock_path.display(), "已释放单实例锁");
    }
}

/// 用默认数据目录的锁文件尝试获取单实例.
///
/// 返回 `None` 表示已经有一个同数据目录的实例在运行, 调用方应当直接退出.
pub fn acquire(args: Vec<String>) -> Result<Option<InstanceGuard>> {
    acquire_at(
        &paths::instance_lock_file(),
        &paths::instance_port_file(),
        args,
    )
}

/// 指定锁文件与端口文件时获取单实例, 便于测试隔离.
pub fn acquire_at(
    lock_path: &Path,
    port_path: &Path,
    args: Vec<String>,
) -> Result<Option<InstanceGuard>> {
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建单实例锁目录失败: {}", parent.display()))?;
    }

    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .with_context(|| format!("打开单实例锁失败: {}", lock_path.display()))?;

    // 更新后重新拉起的新进程会和正在退出的旧进程抢锁, 这里给它一点等待时间,
    // 否则新进程会误判成二次启动而直接退出.
    let relaunching = std::env::var_os(RELAUNCH_ENV).is_some();
    let mut attempts = 0;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if relaunching && attempts < RELAUNCH_MAX_ATTEMPTS => {
                attempts += 1;
                debug!(attempts, "等待旧实例释放单实例锁");
                thread::sleep(RELAUNCH_RETRY_DELAY);
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                forward_to_primary(port_path, &args)?;
                return Ok(None);
            }
            Err(std::fs::TryLockError::Error(err)) => {
                warn!(error = ?err, "单实例锁不可用, 继续以主实例运行");
                break;
            }
        }
    }

    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .context("监听单实例端口失败")?;
    let port = listener.local_addr().context("读取单实例端口失败")?.port();
    std::fs::write(port_path, port.to_string())
        .with_context(|| format!("写入单实例端口文件失败: {}", port_path.display()))?;

    let (sender, receiver) = mpsc::channel();
    let accept_listener = listener.try_clone().context("复制单实例监听句柄失败")?;
    thread::Builder::new()
        .name("instance-listener".to_owned())
        .spawn(move || listen_secondary_launches(accept_listener, sender))
        .context("启动单实例监听线程失败")?;

    let _ = listener;
    info!(port, lock = %lock_path.display(), "已获取单实例锁");

    Ok(Some(InstanceGuard {
        lock,
        lock_path: lock_path.to_path_buf(),
        port_path: port_path.to_path_buf(),
        messages: receiver,
    }))
}

fn listen_secondary_launches(listener: TcpListener, sender: mpsc::Sender<Vec<String>>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            break;
        };
        let Ok(args) = read_args(stream) else {
            continue;
        };
        debug!(args = ?args, "收到二次启动通知");
        if sender.send(args).is_err() {
            break;
        }
    }
}

fn read_args(stream: TcpStream) -> Result<Vec<String>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).context("读取二次启动参数失败")?;
    Ok(split_args(&line))
}

fn forward_to_primary(port_path: &Path, args: &[String]) -> Result<()> {
    let raw = std::fs::read_to_string(port_path)
        .with_context(|| format!("读取单实例端口文件失败: {}", port_path.display()))?;
    let port: u16 = raw.trim().parse().context("单实例端口文件内容不合法")?;
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);

    let addrs = [std::net::SocketAddr::V4(address)];
    let mut stream = TcpStream::connect_timeout(&addrs[0], CONNECT_TIMEOUT)
        .with_context(|| format!("连接已有实例失败: {address}"))?;
    let mut payload = args.join(&ARG_SEPARATOR.to_string());
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .context("转发启动参数失败")?;
    stream.flush().ok();
    info!(port, "已把启动参数转发给已有实例");
    Ok(())
}

fn split_args(line: &str) -> Vec<String> {
    line.trim_end_matches(['\n', '\r'])
        .split(ARG_SEPARATOR)
        .filter(|arg| !arg.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("stcjudge-instance-{name}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        (dir.join("instance.lock"), dir.join("instance.port"))
    }

    #[test]
    fn second_launch_forwards_args_and_exits() {
        let (lock, port) = temp_paths("forward");
        let _ = std::fs::remove_file(&port);
        let primary = acquire_at(&lock, &port, vec!["first".to_owned()])
            .expect("acquire primary")
            .expect("primary guard");

        let secondary = acquire_at(&lock, &port, vec!["--flag".to_owned(), "file.hex".to_owned()])
            .expect("acquire secondary");

        assert!(secondary.is_none());
        let mut received = None;
        for _ in 0..50 {
            if let Some(args) = primary.try_recv() {
                received = Some(args);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(received, Some(vec!["--flag".to_owned(), "file.hex".to_owned()]));
    }

    #[test]
    fn lock_is_released_after_guard_drop() {
        let (lock, port) = temp_paths("release");
        let guard = acquire_at(&lock, &port, Vec::new())
            .expect("acquire primary")
            .expect("primary guard");
        drop(guard);

        let again = acquire_at(&lock, &port, Vec::new()).expect("acquire again");
        assert!(again.is_some());
    }
}
