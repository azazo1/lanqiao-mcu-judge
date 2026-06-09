#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use stcjudge_gui::StcjudgeGuiApp;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    init_tracing();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "stcjudge GUI",
        options,
        Box::new(|ctx| Ok(Box::new(StcjudgeGuiApp::new(ctx)))),
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
