//! 按日期与单文件大小轮转的日志写入器.
//!
//! 每个日志事件独占一次锁, 事件内部的多段写入不会被其他线程插队; 轮转在写入前检查,
//! 归档文件名带 UTC 时间戳, 便于按时间排序与清理.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

/// 单个日志文件的默认大小上限.
pub const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 默认留存的归档文件数量.
pub const DEFAULT_MAX_FILES: usize = 10;

/// 轮转日志写入器.
pub struct RotatingFileWriter {
    state: Mutex<State>,
}

struct State {
    dir: PathBuf,
    stem: String,
    path: PathBuf,
    file: File,
    size: u64,
    day: u64,
    max_bytes: u64,
    max_files: usize,
}

impl RotatingFileWriter {
    /// 打开主日志文件, 不存在时创建, 同时读取已有大小以避免重启后立刻轮转.
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limits(path, DEFAULT_MAX_BYTES, DEFAULT_MAX_FILES)
    }

    pub fn open_with_limits(path: &Path, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        std::fs::create_dir_all(&dir)?;
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "app".to_owned());

        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);

        Ok(Self {
            state: Mutex::new(State {
                dir,
                stem,
                path: path.to_path_buf(),
                file,
                size,
                day: current_day(),
                max_bytes: max_bytes.max(1),
                max_files: max_files.max(1),
            }),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// 刷盘, 退出路径上调用.
    pub fn flush(&self) -> io::Result<()> {
        self.state().file.flush()
    }

    /// 主日志文件路径.
    pub fn path(&self) -> PathBuf {
        self.state().path.clone()
    }
}

impl State {
    fn write_bytes(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.needs_rotation(buf.len() as u64) {
            self.rotate()?;
        }
        self.file.write_all(buf)?;
        self.size = self.size.saturating_add(buf.len() as u64);
        Ok(())
    }

    fn needs_rotation(&self, incoming: u64) -> bool {
        self.size > 0 && (self.size.saturating_add(incoming) > self.max_bytes || self.day != current_day())
    }

    fn rotate(&mut self) -> io::Result<()> {
        let _ = self.file.flush();
        let archived = self
            .dir
            .join(format!("{}-{}.log", self.stem, timestamp_suffix(SystemTime::now())));
        if let Err(err) = std::fs::rename(&self.path, &archived) {
            // 归档失败时清空当前文件继续写, 不能让日志本身把程序带崩.
            eprintln!("轮转日志失败 {}: {err}", self.path.display());
            self.file.set_len(0)?;
            self.size = 0;
            self.day = current_day();
            return Ok(());
        }

        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.size = 0;
        self.day = current_day();
        self.prune_archive();
        Ok(())
    }

    fn prune_archive(&self) {
        let prefix = format!("{}-", self.stem);
        let mut archived: Vec<PathBuf> = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension().is_some_and(|ext| ext == "log")
                        && path
                            .file_name()
                            .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
                })
                .collect(),
            Err(err) => {
                eprintln!("读取日志目录失败 {}: {err}", self.dir.display());
                return;
            }
        };

        if archived.len() <= self.max_files {
            return;
        }
        archived.sort();
        let remove_count = archived.len() - self.max_files;
        for path in archived.into_iter().take(remove_count) {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Write for RotatingFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.state().write_bytes(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.state().file.flush()
    }
}

/// 供 tracing 使用的写入器包装.
///
/// 单独包一层而不是直接给 [`RotatingFileWriter`] 实现 `MakeWriter`, 避免与 tracing 为
/// `Arc<W>` 提供的 `MakeWriter` 实现互相干扰; 每个日志事件独占一次锁, 事件内部的多次
/// 写入不会被其他线程插队.
#[derive(Clone)]
pub struct RotatingFileMakeWriter {
    writer: std::sync::Arc<RotatingFileWriter>,
}

impl RotatingFileMakeWriter {
    pub fn new(writer: std::sync::Arc<RotatingFileWriter>) -> Self {
        Self { writer }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingFileMakeWriter {
    type Writer = RotatingFileGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingFileGuard {
            state: self.writer.state(),
        }
    }
}

/// 单个日志事件的写入句柄, 持有锁直到事件写完.
pub struct RotatingFileGuard<'a> {
    state: MutexGuard<'a, State>,
}

impl Write for RotatingFileGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.state.write_bytes(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.state.file.flush()
    }
}

fn current_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0)
}

/// 归档文件名的 UTC 时间戳, 形如 `20250131-080000`.
pub fn timestamp_suffix(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let seconds_of_day = secs % 86_400;
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    )
}

/// 把自 1970-01-01 起的天数换算成公历年月日 (Howard Hinnant 的 civil_from_days).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stcjudge-log-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn timestamp_suffix_is_stable() {
        let time = UNIX_EPOCH + Duration::from_secs(1_738_310_400);
        assert_eq!(timestamp_suffix(time), "20250131-080000");
    }

    #[test]
    fn oversized_file_rotates_and_keeps_limits() {
        let dir = temp_dir("rotate");
        let path = dir.join("app.log");
        let mut writer = RotatingFileWriter::open_with_limits(&path, 64, 2).expect("open writer");

        for _ in 0..6 {
            writer.write_all(&[b'x'; 40]).expect("write log");
        }
        writer.flush().expect("flush");

        let archived = std::fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("app-"))
            .count();
        assert!(archived <= 2, "归档文件数量应受限制, 实际 {archived}");
        assert!(path.exists());
    }
}
