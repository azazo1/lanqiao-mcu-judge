//! GitHub Release 查询, 版本比较与发布资产匹配.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;

use crate::build_info;

const API_BASE: &str = "https://api.github.com";
const ACCEPT: &str = "application/vnd.github+json";
/// 校验和资产名, 由发布流程生成.
pub const CHECKSUMS_ASSET: &str = "SHA256SUMS";
/// 查询 release 的超时.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// 建立连接的超时.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// release 中的一个资产.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// 可供更新的 release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// 原始 tag, 例如 `v1.2.3`.
    pub tag: String,
    /// 归一化版本, 例如 `1.2.3`.
    pub version: String,
    pub title: String,
    pub notes: String,
    /// 当前平台的更新归档.
    pub asset: ReleaseAsset,
    /// 全部资产的校验和文件.
    pub checksums: ReleaseAsset,
}

/// 构建 HTTP 客户端.
///
/// 每次请求前重新构建, 系统代理只在构建时读取一次, 这样可以跟随运行期代理变更.
pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("stcjudge-gui/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .context("构建 HTTP 客户端失败")
}

/// 查询最新正式 release, 并匹配当前平台的更新资产.
pub async fn fetch_latest() -> Result<ReleaseInfo> {
    if !build_info::supports_in_place_update() {
        bail!(
            "当前平台 {} 没有对应的发布产物, 不支持自动更新",
            build_info::platform_tag()
        );
    }

    let client = http_client()?;
    let url = format!(
        "{API_BASE}/repos/{}/{}/releases/latest",
        build_info::GITHUB_OWNER,
        build_info::GITHUB_REPO
    );
    let response = client
        .get(&url)
        .header(reqwest::header::ACCEPT, ACCEPT)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .context("请求 GitHub release 失败")?;

    let status = response.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("GitHub API 触发速率限制 (HTTP {status}), 请稍后重试");
    }
    if !status.is_success() {
        bail!("查询最新版本失败: HTTP {status}");
    }

    let payload: ReleasePayload = response.json().await.context("解析 GitHub release 响应失败")?;
    let version = normalize_version(&payload.tag_name)
        .with_context(|| format!("release tag 不是合法版本号: {}", payload.tag_name))?;

    let expected_asset = build_info::release_asset_name(&payload.tag_name);
    let mut asset = None;
    let mut checksums = None;
    for candidate in payload.assets {
        if candidate.name == expected_asset {
            asset = Some(ReleaseAsset::from(candidate));
        } else if candidate.name == CHECKSUMS_ASSET {
            checksums = Some(ReleaseAsset::from(candidate));
        }
    }
    let asset = asset.with_context(|| {
        format!(
            "release {} 缺少当前平台资产 {}",
            payload.tag_name, expected_asset
        )
    })?;
    let checksums = checksums
        .with_context(|| format!("release {} 缺少 {CHECKSUMS_ASSET}", payload.tag_name))?;

    Ok(ReleaseInfo {
        tag: payload.tag_name,
        version: version.to_string(),
        title: payload.name.unwrap_or_default(),
        notes: payload.body.unwrap_or_default(),
        asset,
        checksums,
    })
}

/// 归一化运行时版本号.
///
/// 去掉可选的 `v` 前缀, 并截断 `-<commit>` 与 `^<commit>` 形式的构建后缀;
/// `dev-build` 之类无法解析的字符串返回 `None`.
pub fn normalize_version(display: &str) -> Option<Version> {
    let trimmed = display.trim().trim_start_matches('v').trim();
    if trimmed.is_empty() {
        return None;
    }
    Version::parse(strip_build_suffix(trimmed)).ok()
}

fn strip_build_suffix(value: &str) -> &str {
    for separator in ['-', '^'] {
        if let Some((head, tail)) = value.rsplit_once(separator)
            && tail.len() == 7
            && tail.chars().all(|ch| ch.is_ascii_hexdigit())
        {
            return head;
        }
    }
    value
}

/// 候选版本是否严格新于当前版本; 任一侧无法解析出语义化版本时都视为不更新.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (normalize_version(current), normalize_version(candidate)) {
        (Some(current), Some(candidate)) => candidate > current,
        _ => false,
    }
}

#[derive(Debug, Deserialize)]
struct ReleasePayload {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<AssetPayload>,
}

#[derive(Debug, Deserialize)]
struct AssetPayload {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

impl From<AssetPayload> for ReleaseAsset {
    fn from(value: AssetPayload) -> Self {
        Self {
            name: value.name,
            url: value.browser_download_url,
            size: value.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_handles_build_suffixes() {
        assert_eq!(normalize_version("v1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(normalize_version("1.2.3-a1b2c3d").unwrap().to_string(), "1.2.3");
        assert_eq!(normalize_version("v1.2.3^a1b2c3d").unwrap().to_string(), "1.2.3");
        assert_eq!(normalize_version("1.2.3-pre").unwrap().to_string(), "1.2.3-pre");
        assert!(normalize_version("dev-build").is_none());
    }

    #[test]
    fn newer_comparison_ignores_development_builds() {
        assert!(is_newer("v1.2.3", "1.2.4"));
        assert!(is_newer("v1.2.3-a1b2c3d", "1.2.4"));
        assert!(!is_newer("v1.2.3", "1.2.3"));
        assert!(!is_newer("dev-build", "9.9.9"));
        assert!(!is_newer("v1.2.3", "dev-build"));
    }
}
