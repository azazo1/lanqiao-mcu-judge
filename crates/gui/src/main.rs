#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

//! stcjudge GUI 入口.
//!
//! 启动顺序: 日志 -> 单实例 -> 更新服务 -> 窗口. 退出统一走 `StcjudgeGuiApp` 的退出路径,
//! 窗口循环结束后在这里刷盘日志.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use stcjudge_gui::{
    AppHandles, StcjudgeGuiApp, build_info, instance, logging, settings::Settings,
    update::UpdateService, window_icon, window_state,
};

fn main() -> Result<()> {
    logging::init()?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(guard) = instance::acquire(args).context("获取单实例锁失败")? else {
        tracing::info!("已有实例在运行, 本次启动直接退出");
        logging::flush();
        return Ok(());
    };

    let settings = Arc::new(Mutex::new(Settings::load()));
    let geometry = window_state::StartupGeometry::from_settings(&stcjudge_gui::settings::lock(
        &settings,
    ));

    let mut options = eframe::NativeOptions::default();
    options.viewport = options
        .viewport
        .with_title(build_info::APP_TITLE)
        .with_inner_size(match geometry.size {
            Some(size) => [size.x, size.y],
            None => [
                window_state::DEFAULT_SIZE.x,
                window_state::DEFAULT_SIZE.y,
            ],
        })
        .with_min_inner_size([window_state::MIN_SIZE.x, window_state::MIN_SIZE.y])
        .with_maximized(geometry.maximized);
    if let Some(icon) = window_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }

    let update = UpdateService::start(settings.clone());

    let handles = AppHandles {
        instance: Some(guard),
        settings,
        update,
    };
    let result = eframe::run_native(
        build_info::APP_TITLE,
        options,
        Box::new(move |ctx| Ok(Box::new(StcjudgeGuiApp::new(ctx, handles)))),
    );

    logging::flush();
    match result {
        Ok(()) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err.to_string())),
    }
}
