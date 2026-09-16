//! 日志初始化.
//!
//! 文件日志按日期与大小轮转并限制留存数量, 终端同时保留一份输出便于直接观察;
//! `RUST_LOG` 可以覆盖默认的 info 级别, `just debug` 会打开最详细的档位.

pub mod rotation;

use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::paths;
use rotation::{RotatingFileMakeWriter, RotatingFileWriter};

static WRITER: OnceLock<Arc<RotatingFileWriter>> = OnceLock::new();

/// 初始化 tracing, 同时写文件与终端.
///
/// 日志文件不可写时退化为只在终端输出, 不让日志设施挡住应用启动.
pub fn init() -> Result<()> {
    let log_path = paths::log_file();
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let terminal_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    match RotatingFileWriter::open(&log_path) {
        Ok(writer) => {
            let writer = Arc::new(writer);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(RotatingFileMakeWriter::new(writer.clone()));
            tracing_subscriber::registry()
                .with(filter())
                .with(file_layer)
                .with(terminal_layer)
                .try_init()
                .context("初始化日志失败")?;
            let _ = WRITER.set(writer);
        }
        Err(err) => {
            eprintln!(
                "打开日志文件失败 {}: {err}, 本次运行只输出到终端",
                log_path.display()
            );
            tracing_subscriber::fmt()
                .with_env_filter(filter())
                .try_init()
                .map_err(|err| anyhow::anyhow!("初始化日志失败: {err}"))?;
        }
    }

    install_panic_hook();

    tracing::info!(
        version = crate::build_info::display_version(),
        fake_build = crate::build_info::is_fake_build(),
        data_dir = %paths::data_dir().display(),
        log_file = %log_path.display(),
        "stcjudge GUI 启动"
    );

    Ok(())
}

/// 主日志文件路径.
pub fn log_path() -> Option<PathBuf> {
    WRITER.get().map(|writer| writer.path())
}

/// 刷盘日志, 统一退出路径上调用.
pub fn flush() {
    if let Some(writer) = WRITER.get()
        && let Err(err) = writer.flush()
    {
        eprintln!("刷新日志失败: {err}");
    }
}

/// panic 信息写入日志, 便于从日志定位崩溃原因.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "未知位置".to_owned());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "未知 panic 内容".to_owned());
        tracing::error!(target: "stcjudge_gui::panic", location = %location, "panic: {message}");
        previous(info);
    }));
}
