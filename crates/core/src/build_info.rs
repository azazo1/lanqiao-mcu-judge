//! 构建期注入的版本与发布标识.
//!
//! 版本号由 `just dist` 或 CI 通过 `PROJECT_BUILD_VERSION` 注入, 见 `crates/core/build.rs`.
//! 日常开发构建显示 `dev-build`, 这样界面和 `--version` 都不会把开发构建误认成发布版本.

/// 稳定包版本, 取自 `Cargo.toml`, 与发布 tag 严格对齐.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

const BUILD_VERSION: Option<&str> = option_env!("STCJUDGE_BUILD_VERSION");
const FAKE_BUILD: Option<&str> = option_env!("STCJUDGE_FAKE_BUILD");

/// 运行时展示的版本号.
///
/// 发布构建为 `v1.2.3`, 非 tag 提交为 `v1.2.3-a1b2c3d`, 工作区有未提交改动时为 `v1.2.3^a1b2c3d`;
/// 未经发布流程注入版本号时返回 `dev-build`.
pub const fn build_version() -> &'static str {
    match BUILD_VERSION {
        Some(version) => version,
        None => "dev-build",
    }
}

/// 是否为 `just fake-dist` 产出的自动更新测试构建.
pub const fn is_fake_build() -> bool {
    FAKE_BUILD.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_version_is_not_empty() {
        assert!(PACKAGE_VERSION.split('.').count() >= 3);
    }

    #[test]
    fn build_version_falls_back_to_dev_build() {
        assert!(!build_version().is_empty());
    }
}
