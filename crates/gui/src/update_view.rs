//! 更新入口与更新窗口界面.
//!
//! 顶栏右侧平时显示当前版本号, 有新版本或正在下载时替换为可点击的入口; 更新窗口展示
//! 当前状态, release notes, 下载进度与用户可执行的操作.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::{
    build_info,
    settings::Settings,
    update::{UpdatePhase, UpdateService, UpdateSnapshot},
};

/// 更新相关界面的状态.
#[derive(Debug, Default)]
pub struct UpdateView {
    open: bool,
}

impl UpdateView {
    pub fn new() -> Self {
        Self::default()
    }

    /// 顶栏右侧的版本展示区, 返回值为是否打开了更新窗口.
    pub fn status_bar(
        &mut self,
        ui: &mut egui::Ui,
        service: &UpdateService,
        snapshot: &UpdateSnapshot,
    ) -> bool {
        let mut opened = false;
        ui.horizontal(|ui| {
            if snapshot.phase.is_entry() {
                let version = snapshot.version.as_deref().unwrap_or("");
                let text = if version.is_empty() {
                    "有可用更新".to_owned()
                } else {
                    format!("新版本 {version} 可用")
                };
                if ui
                    .add(egui::Label::new(egui::RichText::new(text).strong()).sense(egui::Sense::click()))
                    .on_hover_text("点击查看更新详情")
                    .clicked()
                {
                    self.open = true;
                    opened = true;
                }
            } else {
                ui.label(version_text());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = snapshot.phase.is_busy();
                if ui
                    .add_enabled(!busy, egui::Button::new("检查更新"))
                    .on_hover_text("立即检查是否有新版本")
                    .clicked()
                {
                    service.check();
                    self.open = true;
                    opened = true;
                }
                if let Some(message) = status_hint(snapshot) {
                    ui.label(egui::RichText::new(message).weak());
                }
            });
        });
        opened
    }

    /// 更新窗口.
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        service: &UpdateService,
        snapshot: &UpdateSnapshot,
        settings: &Arc<Mutex<Settings>>,
    ) {
        let mut open = self.open;
        egui::Window::new("软件更新")
            .open(&mut open)
            .resizable(true)
            .default_width(520.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                self.body(ui, service, snapshot, settings);
            });
        self.open = open;
    }

    fn body(
        &mut self,
        ui: &mut egui::Ui,
        service: &UpdateService,
        snapshot: &UpdateSnapshot,
        settings: &Arc<Mutex<Settings>>,
    ) {
        ui.label(format!("当前版本: {}", build_info::display_version()));
        if build_info::is_fake_build() {
            ui.label(
                egui::RichText::new("当前为自动更新测试构建 v0.0.0")
                    .weak(),
            );
        }
        ui.separator();

        ui.label(egui::RichText::new(phase_text(snapshot)).strong());
        if let Some(notice) = &snapshot.notice {
            ui.label(egui::RichText::new(notice).weak());
        }
        if let Some(error) = &snapshot.error {
            ui.colored_label(egui::Color32::from_rgb(180, 55, 55), error);
        }

        if snapshot.phase == UpdatePhase::Downloading {
            let fraction = snapshot.progress_fraction().unwrap_or(0.0);
            ui.add(egui::ProgressBar::new(fraction).show_percentage());
            ui.label(format!(
                "已下载 {} / {}",
                format_bytes(snapshot.received),
                snapshot
                    .total
                    .map(format_bytes)
                    .unwrap_or_else(|| "未知大小".to_owned())
            ));
        }

        if !snapshot.notes.trim().is_empty() {
            ui.separator();
            ui.label("更新说明");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(snapshot.notes.as_str()).wrap());
                });
        }

        ui.separator();
        self.actions(ui, service, snapshot);

        let mut auto_check = settings
            .lock()
            .map(|settings| settings.update.auto_check)
            .unwrap_or(true);
        if ui
            .checkbox(&mut auto_check, "启动时自动检查更新")
            .changed()
        {
            service.set_auto_check(auto_check);
        }
    }

    fn actions(&mut self, ui: &mut egui::Ui, service: &UpdateService, snapshot: &UpdateSnapshot) {
        ui.horizontal_wrapped(|ui| {
            match snapshot.phase {
                UpdatePhase::Available => {
                    if ui.button("立即更新").clicked() {
                        service.download();
                    }
                    if let Some(version) = &snapshot.version
                        && ui
                            .button("跳过此版本")
                            .on_hover_text("静默检查不再提示该版本")
                            .clicked()
                    {
                        service.skip_version(version);
                    }
                }
                UpdatePhase::Failed => {
                    // 归档信息还在说明是下载或替换失败, 可以重试下载; 否则只能重新检查.
                    if snapshot.asset_name.is_some() && ui.button("重试下载").clicked() {
                        service.download();
                    }
                    if ui.button("重新检查").clicked() {
                        service.check();
                    }
                }
                UpdatePhase::Downloading => {
                    if ui.button("取消更新").clicked() {
                        service.cancel_download();
                    }
                }
                UpdatePhase::ReadyToRestart => {
                    if ui
                        .button(egui::RichText::new("重启并更新").strong())
                        .on_hover_text("替换为新版本并重新启动")
                        .clicked()
                    {
                        service.apply();
                    }
                    ui.label(egui::RichText::new("当前版本可继续正常使用").weak());
                }
                UpdatePhase::HandedOff => {
                    ui.spinner();
                    ui.label("正在退出并替换, 请勿手动关闭进程");
                }
                UpdatePhase::DmgOpened => {
                    ui.label("请在打开的镜像中手动完成安装");
                }
                UpdatePhase::Checking => {
                    ui.spinner();
                    ui.label("正在检查更新");
                }
                UpdatePhase::Idle | UpdatePhase::UpToDate => {
                    if ui.button("检查更新").clicked() {
                        service.check();
                    }
                }
            }

            if ui
                .button("查看 Release 页")
                .on_hover_text(build_info::releases_page_url())
                .clicked()
            {
                open_releases_page();
            }
        });
    }
}

fn version_text() -> String {
    format!("{} {}", build_info::APP_TITLE, build_info::display_version())
}

fn status_hint(snapshot: &UpdateSnapshot) -> Option<&'static str> {
    match snapshot.phase {
        UpdatePhase::Checking => Some("正在检查更新"),
        UpdatePhase::Downloading => Some("正在下载更新包"),
        UpdatePhase::ReadyToRestart => Some("新版本已下载, 重启后生效"),
        UpdatePhase::HandedOff => Some("正在退出并替换"),
        UpdatePhase::DmgOpened => Some("请在镜像中手动安装"),
        _ => None,
    }
}

fn phase_text(snapshot: &UpdateSnapshot) -> String {
    match snapshot.phase {
        UpdatePhase::Idle => "尚未检查更新".to_owned(),
        UpdatePhase::Checking => "正在检查更新".to_owned(),
        UpdatePhase::UpToDate => "当前已是最新版本".to_owned(),
        UpdatePhase::Available => match &snapshot.version {
            Some(version) => format!("发现新版本 {version}"),
            None => "发现新版本".to_owned(),
        },
        UpdatePhase::Downloading => "正在下载更新包".to_owned(),
        UpdatePhase::ReadyToRestart => match &snapshot.version {
            Some(version) => format!("新版本 {version} 已下载, 重启后生效"),
            None => "新版本已下载, 重启后生效".to_owned(),
        },
        UpdatePhase::HandedOff => "正在退出并替换".to_owned(),
        UpdatePhase::DmgOpened => "已打开更新镜像".to_owned(),
        UpdatePhase::Failed => "更新失败".to_owned(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn open_releases_page() {
    let url = build_info::releases_page_url();
    if let Err(err) = open_url(&url) {
        tracing::warn!(error = ?err, url, "打开 Release 页面失败");
    }
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command.arg(url).spawn().map(|_| ())
}
