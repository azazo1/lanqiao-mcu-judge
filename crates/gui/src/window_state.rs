//! 主窗口尺寸记忆.
//!
//! 只记忆非全屏, 非最大化时的窗口大小, 启动时恢复; 位置交给系统决定, 不还原上次坐标.
//! 尺寸存在应用数据目录的 `settings.json` 里, 因此 `just debug` 与 fake 构建的隔离实例
//! 各自记忆自己的尺寸. 同目录下运行期窗口尺寸变化结束会即时落盘, 优雅退出时再写一次.
//!
//! 恢复的尺寸必须让整个窗口 (含标题栏) 落在屏幕可用区域内: macOS 上还要排除菜单栏与
//! Dock, 否则窗口底部会藏到 Dock 后面, 贴在底部的状态栏就会被挡住.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use eframe::egui;
use tracing::{debug, info, warn};

use crate::settings::{Settings, lock};

/// 框架默认窗口大小.
pub const DEFAULT_SIZE: egui::Vec2 = egui::vec2(1280.0, 860.0);
/// 允许的最小窗口大小.
pub const MIN_SIZE: egui::Vec2 = egui::vec2(960.0, 640.0);
/// 尺寸变化后等待多久认为调整结束, 避免拖动过程中频繁写盘.
const SETTLE_DELAY: Duration = Duration::from_millis(600);
/// 恢复尺寸时相对显示器预留的余量.
const MONITOR_MARGIN: f32 = 32.0;
/// 小于该尺寸的记录视为无效 (窗口过渡态或损坏的值).
const MIN_RECORDED: f32 = 100.0;

/// 启动时使用的窗口几何.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StartupGeometry {
    /// 上次的非全屏, 非最大化尺寸; 没有可用记录时为 `None`.
    pub size: Option<egui::Vec2>,
    /// 上次是否处于最大化状态.
    pub maximized: bool,
}

impl StartupGeometry {
    /// 从设置里取出可恢复的窗口几何.
    pub fn from_settings(settings: &Settings) -> Self {
        let window = &settings.window;
        let size = match (window.width, window.height) {
            (Some(width), Some(height)) => {
                let size = egui::vec2(width, height);
                usable_size(size).then(|| size.max(MIN_SIZE))
            }
            _ => None,
        };
        Self {
            size,
            maximized: window.maximized,
        }
    }
}

/// 运行期的窗口尺寸跟踪.
#[derive(Debug, Default)]
pub struct WindowState {
    last_seen: Option<egui::Vec2>,
    settle_since: Option<Instant>,
    measured: bool,
}

impl WindowState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧跟踪窗口尺寸; 尺寸稳定一段时间后落盘.
    pub fn track(&mut self, ctx: &egui::Context, settings: &Arc<Mutex<Settings>>) {
        let (inner_rect, outer_rect, maximized, fullscreen, monitor_size) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport.inner_rect,
                viewport.outer_rect,
                viewport.maximized.unwrap_or(false),
                viewport.fullscreen.unwrap_or(false),
                viewport.monitor_size,
            )
        });

        if maximized || fullscreen {
            // 最大化与全屏的尺寸不是有效尺寸, 只单独记录最大化标志.
            self.last_seen = None;
            self.settle_since = None;
            self.persist(ctx, settings, None, maximized);
            return;
        }

        let Some(inner_rect) = inner_rect else {
            return;
        };
        let size = inner_rect.size();

        // 首次拿到窗口几何时记录一次, 并确认整个窗口 (含标题栏) 仍落在屏幕可用区域内.
        if !self.measured {
            self.measured = true;
            if let Some(outer_rect) = outer_rect {
                let chrome = outer_rect.size() - size;
                let outer_size = size + chrome;
                let area = available_area(monitor_size);
                if let Some((area, margin)) = area {
                    debug!(
                        monitor = ?monitor_size,
                        area = ?area,
                        inner = ?size,
                        outer = ?outer_size,
                        "窗口几何"
                    );
                    if exceeds_area(outer_size, area, margin) {
                        let clamped = (clamp_to_area(outer_size, area, margin) - chrome).max(MIN_SIZE);
                        info!(from = ?size, to = ?clamped, "恢复的窗口尺寸放不下, 已夹取到屏幕可用区域内");
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(clamped));
                        return;
                    }
                }
            }
        }

        if !usable_size(size) {
            return;
        }

        match self.last_seen {
            Some(last) if is_same_size(last, size) => {
                let settled = self
                    .settle_since
                    .is_some_and(|since| since.elapsed() >= SETTLE_DELAY);
                if settled {
                    self.settle_since = None;
                    self.persist(ctx, settings, Some(size), false);
                } else {
                    ctx.request_repaint_after(SETTLE_DELAY);
                }
            }
            _ => {
                self.last_seen = Some(size);
                self.settle_since = Some(Instant::now());
                ctx.request_repaint_after(SETTLE_DELAY);
            }
        }
    }

    /// 退出路径上再写一次, 避免异常退出丢失本次调整.
    pub fn persist_now(&mut self, ctx: &egui::Context, settings: &Arc<Mutex<Settings>>) {
        let (size, maximized, fullscreen) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport.inner_rect.map(|rect| rect.size()),
                viewport.maximized.unwrap_or(false),
                viewport.fullscreen.unwrap_or(false),
            )
        });

        let size = if maximized || fullscreen {
            None
        } else {
            size.filter(|size| usable_size(*size))
        };
        self.persist(ctx, settings, size, maximized);
    }

    fn persist(
        &mut self,
        ctx: &egui::Context,
        settings: &Arc<Mutex<Settings>>,
        size: Option<egui::Vec2>,
        maximized: bool,
    ) {
        let mut settings = lock(settings);
        let window = &mut settings.window;
        let mut changed = false;

        if window.maximized != maximized {
            window.maximized = maximized;
            changed = true;
        }
        if let Some(size) = size
            && (window.width != Some(size.x) || window.height != Some(size.y))
        {
            window.width = Some(size.x);
            window.height = Some(size.y);
            changed = true;
        }

        if !changed {
            return;
        }
        if let Err(err) = settings.save() {
            warn!(error = ?err, "保存窗口尺寸失败");
            return;
        }
        debug!(width = ?settings.window.width, height = ?settings.window.height, maximized, "已记住窗口尺寸");
        let _ = ctx;
    }
}

/// 尺寸是否足够大, 可作为可记忆的窗口大小.
fn usable_size(size: egui::Vec2) -> bool {
    size.x >= MIN_RECORDED && size.y >= MIN_RECORDED
}

/// 窗口可用的屏幕区域: 优先使用排除菜单栏与 Dock 的工作区, 取不到时退化为显示器尺寸.
///
/// 返回区域尺寸与需要额外预留的边距.
fn available_area(monitor_size: Option<egui::Vec2>) -> Option<(egui::Vec2, f32)> {
    if let Some(area) = work_area() {
        return Some((area, 0.0));
    }
    monitor_size.map(|size| (size, MONITOR_MARGIN))
}

/// 尺寸是否超出可用区域.
fn exceeds_area(size: egui::Vec2, area: egui::Vec2, margin: f32) -> bool {
    size.x > area.x - margin || size.y > area.y - margin
}

/// 把尺寸夹取到可用区域内, 但不会小于允许的最小窗口大小.
fn clamp_to_area(size: egui::Vec2, area: egui::Vec2, margin: f32) -> egui::Vec2 {
    egui::vec2(
        size.x.min((area.x - margin).max(MIN_SIZE.x)),
        size.y.min((area.y - margin).max(MIN_SIZE.y)),
    )
}

/// 屏幕可用区域 (不含菜单栏与 Dock), 单位为 egui 点.
#[cfg(target_os = "macos")]
fn work_area() -> Option<egui::Vec2> {
    let mtm = objc2::MainThreadMarker::new()?;
    let screen = objc2_app_kit::NSScreen::mainScreen(mtm)?;
    let frame = screen.visibleFrame();
    Some(egui::vec2(
        frame.size.width as f32,
        frame.size.height as f32,
    ))
}

/// 其他平台无法直接查询工作区, 由调用方退回显示器尺寸.
#[cfg(not(target_os = "macos"))]
fn work_area() -> Option<egui::Vec2> {
    None
}

fn is_same_size(left: egui::Vec2, right: egui::Vec2) -> bool {
    (left.x - right.x).abs() < 0.5 && (left.y - right.y).abs() < 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_window(width: Option<f32>, height: Option<f32>, maximized: bool) -> Settings {
        let mut settings = Settings::default();
        settings.window.width = width;
        settings.window.height = height;
        settings.window.maximized = maximized;
        settings
    }

    #[test]
    fn startup_geometry_without_record_uses_defaults() {
        let geometry = StartupGeometry::from_settings(&Settings::default());
        assert_eq!(geometry.size, None);
        assert!(!geometry.maximized);
    }

    #[test]
    fn startup_geometry_keeps_recorded_size_and_maximized_flag() {
        let settings = settings_with_window(Some(1100.0), Some(700.0), true);
        let geometry = StartupGeometry::from_settings(&settings);
        assert_eq!(geometry.size, Some(egui::vec2(1100.0, 700.0)));
        assert!(geometry.maximized);
    }

    #[test]
    fn startup_geometry_lifts_too_small_size_to_minimum() {
        let settings = settings_with_window(Some(420.0), Some(320.0), false);
        let geometry = StartupGeometry::from_settings(&settings);
        assert_eq!(geometry.size, Some(MIN_SIZE));
    }

    #[test]
    fn startup_geometry_ignores_degenerate_size() {
        let settings = settings_with_window(Some(0.0), Some(0.0), false);
        assert_eq!(StartupGeometry::from_settings(&settings).size, None);

        let settings = settings_with_window(Some(f32::NAN), Some(500.0), false);
        assert_eq!(StartupGeometry::from_settings(&settings).size, None);
    }

    #[test]
    fn clamping_keeps_window_inside_available_area() {
        // 工作区 (已排除菜单栏与 Dock), 例如 1280x800 屏幕上的 1280x710.
        let area = egui::vec2(1280.0, 710.0);
        assert!(exceeds_area(egui::vec2(1280.0, 815.0), area, 0.0));
        let clamped = clamp_to_area(egui::vec2(1280.0, 815.0), area, 0.0);
        assert!(clamped.y <= area.y);
        assert_eq!(clamped.x, 1280.0);
        assert!(!exceeds_area(egui::vec2(1200.0, 700.0), area, 0.0));
        assert_eq!(
            clamp_to_area(egui::vec2(1200.0, 700.0), area, 0.0),
            egui::vec2(1200.0, 700.0)
        );
    }

    #[test]
    fn clamping_falls_back_to_monitor_size() {
        let (area, margin) = available_area(Some(egui::vec2(1440.0, 900.0))).expect("area");
        assert_eq!(area, egui::vec2(1440.0, 900.0));
        assert_eq!(margin, MONITOR_MARGIN);
        assert!(available_area(None).is_none());
    }
}
