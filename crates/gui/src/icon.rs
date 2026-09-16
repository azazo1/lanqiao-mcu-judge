//! 应用图标.

use std::sync::Arc;

use egui::IconData;

/// 编译进二进制的应用图标, 供窗口与任务栏使用.
const ICON_PNG: &[u8] = include_bytes!("../../../assets/app-icon.png");

/// 窗口图标; 图标损坏时返回 `None`, 不影响程序启动.
pub fn window_icon() -> Option<Arc<IconData>> {
    let image = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(Arc::new(IconData {
        rgba: image.into_raw(),
        width,
        height,
    }))
}
