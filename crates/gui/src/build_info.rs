//! 应用标识, 版本展示与发布资产命名约定.
//!
//! 版本号来自构建期注入 (见 `stcjudge::build_info`), 资产命名与
//! `create-github-release-flow` 的约定一致: `<app>-<version>-<platform>-<arch>.<ext>`.

use stcjudge::build_info;

/// 应用名, 同时作为数据目录名和发布资产前缀.
pub const APP_NAME: &str = "stcjudge-gui";

/// 窗口标题与关于信息使用的展示名.
pub const APP_TITLE: &str = "stcjudge GUI";

/// 发布仓库.
pub const GITHUB_OWNER: &str = "azazo1";
pub const GITHUB_REPO: &str = "lanqiao-mcu-judge";

/// 当前构建展示的版本号.
///
/// 发布构建形如 `v1.2.3`, 非 tag 提交形如 `v1.2.3-a1b2c3d`, 工作区有未提交改动时形如
/// `v1.2.3^a1b2c3d`; 普通开发构建为 `dev-build`.
pub fn display_version() -> &'static str {
    build_info::build_version()
}

/// 是否为自动更新测试构建.
pub fn is_fake_build() -> bool {
    build_info::is_fake_build()
}

/// 发布资产中的平台与架构标签, 例如 `macos-aarch64`.
///
/// 没有对应发布产物的平台返回 `unsupported`, 自动更新会据此拒绝运行.
pub fn platform_tag() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macos-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("windows", "x86_64") => "windows-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        _ => "unsupported",
    }
}

/// 当前平台需要的更新资产扩展名.
pub fn asset_extension() -> &'static str {
    match std::env::consts::OS {
        "macos" => "dmg",
        "windows" => "zip",
        _ => "tar.gz",
    }
}

/// 按约定拼出当前平台的更新资产名, 例如 `stcjudge-gui-1.2.3-macos-aarch64.dmg`.
pub fn release_asset_name(version: &str) -> String {
    format!(
        "{APP_NAME}-{}-{}.{}",
        version.trim_start_matches('v'),
        platform_tag(),
        asset_extension()
    )
}

/// Release 页面地址.
pub fn releases_page_url() -> String {
    format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases")
}

/// 当前平台是否支持应用内自动安装.
pub fn supports_in_place_update() -> bool {
    platform_tag() != "unsupported"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_asset_name_keeps_convention() {
        let name = release_asset_name("v1.2.3");
        assert!(name.starts_with("stcjudge-gui-1.2.3-"), "{name}");
        assert!(name.ends_with(asset_extension()), "{name}");
    }

    #[test]
    fn display_version_is_never_empty() {
        assert!(!display_version().is_empty());
    }
}
