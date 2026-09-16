//! 终端信号与统一退出路径.
//!
//! 从终端启动的实例收到 Ctrl+C 时只置位退出请求并唤醒界面, 由 UI 线程走统一退出路径收尾,
//! 避免进程被信号直接杀死而遗留后台任务或丢失日志.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result};
use tracing::warn;

/// Ctrl+C 退出请求.
#[derive(Clone, Default)]
pub struct ExitSignal {
    requested: Arc<AtomicBool>,
}

impl ExitSignal {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 SIGINT 处理; 平台不支持时只记录警告, 不影响正常退出.
    pub fn install(&self, ctx: eframe::egui::Context) -> Result<()> {
        let requested = self.requested.clone();
        ctrlc::set_handler(move || {
            requested.store(true, Ordering::SeqCst);
            ctx.request_repaint();
        })
        .context("注册 Ctrl+C 处理失败")
    }

    /// 是否收到过退出请求.
    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }
}

/// 注册信号处理失败时只提示一次.
pub fn install_or_warn(signal: &ExitSignal, ctx: eframe::egui::Context) {
    if let Err(err) = signal.install(ctx) {
        warn!(error = ?err, "无法注册 Ctrl+C 处理, 终端按键退出不可用");
    }
}
