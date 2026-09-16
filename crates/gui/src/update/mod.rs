//! 自动更新的状态机与后台服务.
//!
//! 状态流转:
//!
//! ```text
//! Idle -> Checking -> (UpToDate | Available | Failed)
//! Available -> Downloading -> (ReadyToRestart | Available | Failed)
//! ReadyToRestart -> (HandedOff | DmgOpened)
//! ```
//!
//! 下载完成只走到 `ReadyToRestart`, 替换, 重启这些动作必须由用户点击 "重启并更新" 触发;
//! 检查, 下载与替换都在后台任务里执行, UI 只读取不可变快照渲染.

mod download;
mod github;
#[cfg(not(target_os = "macos"))]
mod install;
#[cfg(target_os = "macos")]
mod macos_handoff;
mod notification;

use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{debug, info, warn};

use crate::{build_info, paths, settings::Settings};

pub use github::ReleaseInfo;

/// 启动后延迟多久做静默检查.
const AUTO_CHECK_DELAY: Duration = Duration::from_secs(5);

type SharedState = Arc<Mutex<UpdateSnapshot>>;
type SharedSettings = Arc<Mutex<Settings>>;

/// 更新流程所处阶段.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    ReadyToRestart,
    HandedOff,
    DmgOpened,
    Failed,
}

impl UpdatePhase {
    /// 状态栏是否应把版本号换成可点击的更新入口.
    pub fn is_entry(self) -> bool {
        matches!(
            self,
            Self::Available
                | Self::Downloading
                | Self::ReadyToRestart
                | Self::HandedOff
                | Self::DmgOpened
                | Self::Failed
        )
    }

    /// 是否正在后台忙碌, 用于决定是否需要持续重绘.
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Checking | Self::Downloading)
    }
}

/// UI 读取的更新状态快照.
#[derive(Debug, Clone, Default)]
pub struct UpdateSnapshot {
    pub phase: UpdatePhase,
    /// 新版本号, 不带 `v` 前缀.
    pub version: Option<String>,
    /// 原始 release tag.
    pub tag: Option<String>,
    /// release notes.
    pub notes: String,
    /// 当前平台待安装的归档名.
    pub asset_name: Option<String>,
    pub received: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    /// 一次性提示, 例如上次替换失败的原因.
    pub notice: Option<String>,
}

impl UpdateSnapshot {
    /// 已下载字节数占总大小的比例.
    pub fn progress_fraction(&self) -> Option<f32> {
        let total = self.total?;
        if total == 0 {
            return None;
        }
        Some((self.received as f32 / total as f32).clamp(0.0, 1.0))
    }

    /// 本地待安装归档路径.
    pub fn archive_path(&self) -> Option<PathBuf> {
        self.asset_name
            .as_ref()
            .map(|name| paths::update_dir().join(name))
    }
}

/// 后台服务收到的命令.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateCommand {
    StartupCheck,
    Check { silent: bool },
    Download,
    CancelDownload,
    Apply,
}

/// 自动更新服务句柄.
pub struct UpdateService {
    state: SharedState,
    settings: SharedSettings,
    sender: Option<UnboundedSender<UpdateCommand>>,
    exit_requested: Arc<AtomicBool>,
}

impl UpdateService {
    /// 启动后台服务; 启动时会先恢复上次下载好的更新包并清理残留文件.
    pub fn start(settings: SharedSettings) -> Self {
        let state: SharedState = Arc::new(Mutex::new(UpdateSnapshot::default()));
        let release: Arc<Mutex<Option<ReleaseInfo>>> = Arc::new(Mutex::new(None));
        let exit_requested = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::unbounded_channel();

        let thread_state = state.clone();
        let thread_release = release.clone();
        let thread_settings = settings.clone();
        let thread_exit = exit_requested.clone();
        let startup_sender = sender.clone();
        let spawn_result = thread::Builder::new()
            .name("update-service".to_owned())
            .spawn(move || {
                run_service(
                    thread_state,
                    thread_release,
                    thread_settings,
                    thread_exit,
                    receiver,
                    startup_sender,
                )
            });

        if let Err(err) = spawn_result {
            warn!(error = ?err, "启动更新服务线程失败, 本次运行不检查更新");
        }

        Self {
            state,
            settings,
            sender: Some(sender),
            exit_requested,
        }
    }

    /// 当前状态快照.
    pub fn snapshot(&self) -> UpdateSnapshot {
        lock(&self.state).clone()
    }

    /// 手动检查更新.
    pub fn check(&self) {
        self.send(UpdateCommand::Check { silent: false });
    }

    /// 开始下载当前可用版本.
    pub fn download(&self) {
        self.send(UpdateCommand::Download);
    }

    /// 取消正在进行的下载, 已下载的部分保留供下次续传.
    pub fn cancel_download(&self) {
        self.send(UpdateCommand::CancelDownload);
    }

    /// 用户点击 "重启并更新".
    pub fn apply(&self) {
        self.send(UpdateCommand::Apply);
    }

    /// 是否已经交出替换工作, 应用应当走统一退出路径.
    pub fn exit_requested(&self) -> bool {
        self.exit_requested.load(Ordering::SeqCst)
    }

    /// 记录被用户跳过的版本.
    pub fn skip_version(&self, version: &str) {
        info!(version, "已跳过该版本的更新提示");
        let mut settings = lock(&self.settings);
        settings.update.skipped_version = Some(version.to_owned());
        if let Err(err) = settings.save() {
            warn!(error = ?err, "保存跳过版本设置失败");
        }
    }

    /// 设置启动时自动检查更新, 并持久化.
    pub fn set_auto_check(&self, enabled: bool) {
        info!(enabled, "已更新启动自动检查更新设置");
        let mut settings = lock(&self.settings);
        settings.update.auto_check = enabled;
        if let Err(err) = settings.save() {
            warn!(error = ?err, "保存自动检查设置失败");
        }
    }

    /// 停止后台服务; 更新线程会在当前任务结束后自行退出.
    pub fn shutdown(&mut self) {
        self.sender = None;
    }

    fn send(&self, command: UpdateCommand) {
        let Some(sender) = &self.sender else {
            return;
        };
        if let Err(err) = sender.send(command) {
            debug!(error = ?err, "更新服务已停止, 忽略命令");
        }
    }
}

fn run_service(
    state: SharedState,
    release_slot: Arc<Mutex<Option<ReleaseInfo>>>,
    settings: SharedSettings,
    exit_requested: Arc<AtomicBool>,
    mut receiver: mpsc::UnboundedReceiver<UpdateCommand>,
    startup_sender: UnboundedSender<UpdateCommand>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            warn!(error = ?err, "创建更新服务运行时失败");
            return;
        }
    };

    runtime.block_on(async move {
        restore_local_update(&state);

        tokio::spawn(async move {
            tokio::time::sleep(AUTO_CHECK_DELAY).await;
            let _ = startup_sender.send(UpdateCommand::StartupCheck);
        });

        let mut download_task: Option<tokio::task::JoinHandle<()>> = None;
        while let Some(command) = receiver.recv().await {
            let downloading = lock(&state).phase == UpdatePhase::Downloading;
            match command {
                UpdateCommand::StartupCheck | UpdateCommand::Check { .. } if downloading => {
                    debug!("下载进行中, 跳过本次更新检查");
                }
                UpdateCommand::StartupCheck => {
                    if auto_check_enabled(&settings) {
                        run_check(&state, &release_slot, &settings, true).await;
                    } else {
                        debug!("已关闭启动自动检查更新");
                    }
                }
                UpdateCommand::Check { silent } => {
                    run_check(&state, &release_slot, &settings, silent).await;
                }
                UpdateCommand::Download => {
                    if let Some(release) = lock(&release_slot).clone() {
                        if let Some(task) = download_task.take() {
                            task.abort();
                        }
                        download_task = Some(tokio::spawn(download_flow(state.clone(), release)));
                    }
                }
                UpdateCommand::CancelDownload => {
                    if let Some(task) = download_task.take() {
                        task.abort();
                        info!("已取消下载更新包");
                    }
                    back_to_available(&state);
                }
                UpdateCommand::Apply => {
                    download_task = None;
                    apply_flow(&state, &exit_requested).await;
                }
            }
        }
        debug!("更新服务已停止");
    });
}

/// 恢复上次已下载的更新包, 清理残留文件.
fn restore_local_update(state: &SharedState) {
    let update_dir = paths::update_dir();
    #[cfg(target_os = "macos")]
    let replace_result = {
        macos_handoff::cleanup_on_start(&update_dir);
        macos_handoff::take_result(&update_dir)
    };
    #[cfg(not(target_os = "macos"))]
    let replace_result = {
        install::cleanup_stale_backup();
        None
    };

    let pending = download::load_pending(&update_dir);
    if let Some((pending, archive)) = pending {
        info!(asset = %pending.asset, "发现上次已下载的更新包, 直接进入等待重启状态");
        let mut snapshot = lock(state);
        snapshot.phase = UpdatePhase::ReadyToRestart;
        snapshot.version = Some(pending.version);
        snapshot.tag = Some(pending.tag);
        snapshot.notes = pending.notes;
        snapshot.total = Some(pending.size);
        snapshot.received = pending.size;
        snapshot.asset_name = Some(pending.asset);
        snapshot.notice = Some(format!(
            "上次已下载 {} 的新版本, 重启后生效",
            archive.display()
        ));
    }

    if let Some(message) = replace_result {
        let mut snapshot = lock(state);
        if snapshot.phase != UpdatePhase::ReadyToRestart {
            snapshot.phase = UpdatePhase::Failed;
        }
        snapshot.error = Some(message);
    }
}

async fn run_check(
    state: &SharedState,
    release_slot: &Arc<Mutex<Option<ReleaseInfo>>>,
    settings: &SharedSettings,
    silent: bool,
) {
    // 记住进入检查前的状态, 检查失败或发现本地已有待重启版本时要恢复回去.
    let previous = lock(state).clone();
    set_phase(state, UpdatePhase::Checking, |snapshot| {
        snapshot.error = None;
        snapshot.notice = None;
    });

    let release = match github::fetch_latest().await {
        Ok(release) => release,
        Err(err) => {
            if silent {
                debug!(error = %error_chain(&err), "静默检查更新失败");
                match previous.phase {
                    UpdatePhase::Available
                    | UpdatePhase::Downloading
                    | UpdatePhase::ReadyToRestart => {
                        set_phase(state, previous.phase, |_| {});
                    }
                    _ => set_phase(state, UpdatePhase::Idle, |_| {}),
                }
            } else {
                warn!(error = %error_chain(&err), "检查更新失败");
                set_phase(state, UpdatePhase::Failed, |snapshot| {
                    snapshot.error = Some(err.to_string());
                });
            }
            return;
        }
    };

    let current = build_info::display_version();
    if !github::is_newer(current, &release.version) {
        debug!(current, latest = %release.version, "当前已是最新版本");
        if previous.phase == UpdatePhase::ReadyToRestart {
            set_phase(state, UpdatePhase::ReadyToRestart, |_| {});
            return;
        }
        set_phase(state, UpdatePhase::UpToDate, |snapshot| {
            snapshot.version = None;
            snapshot.tag = None;
            snapshot.notes.clear();
            snapshot.asset_name = None;
            snapshot.received = 0;
            snapshot.total = None;
            snapshot.error = None;
        });
        return;
    }

    // 本地已经下载好的版本不要因为一次检查退回 "可更新", 否则用户会被要求重复下载.
    if previous.phase == UpdatePhase::ReadyToRestart
        && previous.version.as_deref() == Some(release.version.as_str())
    {
        debug!(version = %release.version, "本地已下载该版本, 保持等待重启状态");
        set_phase(state, UpdatePhase::ReadyToRestart, |_| {});
        return;
    }

    let skipped = lock(settings).update.skipped_version.clone();
    let skipped_match = skipped.as_deref() == Some(release.version.as_str());
    info!(current, available = %release.version, silent, skipped = skipped_match, "发现新版本");

    let version = release.version.clone();
    let tag = release.tag.clone();
    let notes = release.notes.clone();
    let asset_name = release.asset.name.clone();
    let total = Some(release.asset.size);
    set_phase(state, UpdatePhase::Available, |snapshot| {
        snapshot.version = Some(version);
        snapshot.tag = Some(tag);
        snapshot.notes = notes;
        snapshot.asset_name = Some(asset_name);
        snapshot.received = 0;
        snapshot.total = total;
        snapshot.error = None;
        snapshot.notice = None;
    });
    *lock(release_slot) = Some(release.clone());

    if silent && !skipped_match {
        notification::notify_update_available(&release.version);
    }
}

async fn download_flow(state: SharedState, release: ReleaseInfo) {
    set_phase(&state, UpdatePhase::Downloading, |snapshot| {
        snapshot.received = 0;
        snapshot.total = Some(release.asset.size);
        snapshot.error = None;
    });

    let update_dir = paths::update_dir();
    let client = match github::http_client() {
        Ok(client) => client,
        Err(err) => {
            fail_download(&state, err);
            return;
        }
    };

    let progress_state = state.clone();
    let mut progress = move |received: u64, total: Option<u64>| {
        let mut snapshot = lock(&progress_state);
        snapshot.received = received;
        if total.is_some() {
            snapshot.total = total;
        }
    };

    let result = download::download_archive(&client, &release, &update_dir, &mut progress).await;
    match result {
        Ok(pending) => {
            info!(asset = %pending.asset, "更新包已就绪, 等待用户重启");
            set_phase(&state, UpdatePhase::ReadyToRestart, |snapshot| {
                snapshot.received = pending.size;
                snapshot.total = Some(pending.size);
                snapshot.notes = pending.notes;
                snapshot.error = None;
                snapshot.notice = None;
            });
        }
        Err(err) => fail_download(&state, err),
    }
}

fn fail_download(state: &SharedState, err: anyhow::Error) {
    warn!(error = %error_chain(&err), "下载更新包失败");
    set_phase(state, UpdatePhase::Failed, |snapshot| {
        snapshot.error = Some(err.to_string());
    });
}

fn back_to_available(state: &SharedState) {
    let mut snapshot = lock(state);
    if snapshot.phase == UpdatePhase::Downloading {
        snapshot.phase = UpdatePhase::Available;
        snapshot.received = 0;
    }
}

async fn apply_flow(state: &SharedState, exit_requested: &Arc<AtomicBool>) {
    let snapshot = lock(state).clone();
    let Some(asset_name) = snapshot.asset_name.clone() else {
        return;
    };
    let archive = paths::update_dir().join(&asset_name);
    if !archive.is_file() {
        set_phase(state, UpdatePhase::Failed, |snapshot| {
            snapshot.error = Some("更新包已不存在, 请重新下载".to_owned());
        });
        return;
    }

    #[cfg(target_os = "macos")]
    {
        let update_dir = paths::update_dir();
        match macos_handoff::spawn_apply(&archive, &update_dir) {
            Ok(Some(paths)) => {
                info!(bundle = %paths.bundle.display(), "已交接替换脚本, 应用即将退出");
                set_phase(state, UpdatePhase::HandedOff, |snapshot| {
                    snapshot.error = None;
                    snapshot.notice =
                        Some("正在退出并替换, 请勿手动关闭进程".to_owned());
                });
                exit_requested.store(true, Ordering::SeqCst);
            }
            Ok(None) => {
                // 便携运行: 没有可替换的 bundle, 退回打开镜像手动安装.
                match Command::new("open").arg(&archive).spawn() {
                    Ok(_) => {
                        info!(dmg = %archive.display(), "已打开更新镜像, 等待用户手动安装");
                        set_phase(state, UpdatePhase::DmgOpened, |snapshot| {
                            snapshot.error = None;
                            snapshot.notice = Some(
                                "当前为便携运行, 已打开更新镜像, 请手动把应用拖入 应用程序 目录"
                                    .to_owned(),
                            );
                        });
                    }
                    Err(err) => {
                        warn!(error = ?err, "打开更新镜像失败");
                        set_phase(state, UpdatePhase::Failed, |snapshot| {
                            snapshot.error = Some(format!("打开更新镜像失败: {err}"));
                        });
                    }
                }
            }
            Err(err) => {
                warn!(error = ?err, "交接替换脚本失败");
                set_phase(state, UpdatePhase::Failed, |snapshot| {
                    snapshot.error = Some(err.to_string());
                });
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let work_dir = paths::update_dir();
        let restart_args: Vec<String> = std::env::args().skip(1).collect();
        let result = tokio::task::spawn_blocking(move || {
            install::apply_archive(&archive, &work_dir, &restart_args)
        })
        .await;

        match result {
            Ok(Ok(program)) => {
                info!(program = %program.display(), "已替换为新版本, 应用即将退出");
                set_phase(state, UpdatePhase::HandedOff, |snapshot| {
                    snapshot.error = None;
                    snapshot.notice = Some("已启动新版本, 当前进程即将退出".to_owned());
                });
                exit_requested.store(true, Ordering::SeqCst);
            }
            Ok(Err(err)) => {
                warn!(error = %error_chain(&err), "替换更新失败");
                set_phase(state, UpdatePhase::Failed, |snapshot| {
                    snapshot.error = Some(err.to_string());
                });
            }
            Err(err) => {
                warn!(error = ?err, "替换更新任务失败");
                set_phase(state, UpdatePhase::Failed, |snapshot| {
                    snapshot.error = Some(format!("替换更新任务失败: {err}"));
                });
            }
        }
    }
}

fn auto_check_enabled(settings: &SharedSettings) -> bool {
    lock(settings).update.auto_check
}

fn set_phase(state: &SharedState, phase: UpdatePhase, update: impl FnOnce(&mut UpdateSnapshot)) {
    let mut snapshot = lock(state);
    snapshot.phase = phase;
    update(&mut snapshot);
}

/// 把 anyhow 错误展开成单行链路, 便于在日志里定位根因.
fn error_chain(err: &anyhow::Error) -> String {
    err.chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_fraction_is_bounded() {
        let snapshot = UpdateSnapshot {
            received: 50,
            total: Some(200),
            ..Default::default()
        };
        assert_eq!(snapshot.progress_fraction(), Some(0.25));
        assert_eq!(UpdateSnapshot::default().progress_fraction(), None);
    }

    #[test]
    fn phases_mark_ui_entry_and_busy() {
        assert!(UpdatePhase::ReadyToRestart.is_entry());
        assert!(!UpdatePhase::Idle.is_entry());
        assert!(UpdatePhase::Downloading.is_busy());
        assert!(!UpdatePhase::Available.is_busy());
    }
}
