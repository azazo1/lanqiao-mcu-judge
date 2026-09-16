//! 更新归档的下载, 断点续传与 SHA256 校验.
//!
//! 归档先落到 `<update_dir>/<name>.part`, 校验通过后才改名成正式文件, 并写下
//! `pending.json`. 落盘即下载流程的终点, 是否替换与重启完全由用户决定.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use super::github::ReleaseInfo;

/// 本地保存的校验和文件名.
pub const CHECKSUMS_FILE: &str = "SHA256SUMS";
/// 已下载待重启的更新记录.
pub const PENDING_FILE: &str = "pending.json";

/// 已经下载完成, 等待用户重启生效的更新包.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub tag: String,
    pub version: String,
    pub asset: String,
    pub notes: String,
    pub size: u64,
}

/// 更新目录中的记录文件路径.
pub fn pending_path(update_dir: &Path) -> PathBuf {
    update_dir.join(PENDING_FILE)
}

/// 读取本地待重启记录; 归档缺失或摘要不匹配时返回 `None`.
pub fn load_pending(update_dir: &Path) -> Option<(PendingUpdate, PathBuf)> {
    let raw = fs::read_to_string(pending_path(update_dir)).ok()?;
    let pending: PendingUpdate = serde_json::from_str(&raw).ok()?;
    let archive = update_dir.join(&pending.asset);
    if !archive.is_file() {
        return None;
    }

    let expected = expected_digest(update_dir, &pending.asset).ok()?;
    match sha256_file(&archive) {
        Ok(digest) if digest.eq_ignore_ascii_case(&expected) => Some((pending, archive)),
        Ok(digest) => {
            warn!(asset = %pending.asset, digest, expected, "本地更新包摘要不匹配, 丢弃");
            cleanup(update_dir);
            None
        }
        Err(err) => {
            warn!(asset = %pending.asset, error = ?err, "校验本地更新包失败, 丢弃");
            cleanup(update_dir);
            None
        }
    }
}

/// 删除更新目录中已失效的归档与临时文件.
pub fn cleanup(update_dir: &Path) {
    let _ = fs::remove_file(pending_path(update_dir));
    let Ok(entries) = fs::read_dir(update_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".part") || (path.is_file() && name != CHECKSUMS_FILE) {
            let _ = fs::remove_file(path);
        }
    }
}

/// 下载校验和文件并取出目标归档的期望摘要.
pub async fn fetch_expected_digest(
    client: &reqwest::Client,
    release: &ReleaseInfo,
    update_dir: &Path,
) -> Result<String> {
    let body = client
        .get(&release.checksums.url)
        .send()
        .await
        .context("下载 SHA256SUMS 失败")?
        .error_for_status()
        .context("下载 SHA256SUMS 失败")?
        .text()
        .await
        .context("读取 SHA256SUMS 失败")?;

    let digest = parse_checksums(&body, &release.asset.name)?;
    fs::create_dir_all(update_dir)
        .with_context(|| format!("创建更新目录失败: {}", update_dir.display()))?;
    fs::write(update_dir.join(CHECKSUMS_FILE), body.as_bytes())
        .with_context(|| format!("保存 {} 失败", update_dir.join(CHECKSUMS_FILE).display()))?;
    Ok(digest)
}

/// 下载归档, 支持断点续传; `progress` 收到已下载字节数与总字节数.
pub async fn download_archive(
    client: &reqwest::Client,
    release: &ReleaseInfo,
    update_dir: &Path,
    progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
) -> Result<PendingUpdate> {
    let expected = fetch_expected_digest(client, release, update_dir).await?;
    fs::create_dir_all(update_dir)
        .with_context(|| format!("创建更新目录失败: {}", update_dir.display()))?;

    let part = update_dir.join(format!("{}.part", release.asset.name));
    let resumed_from = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);

    let mut request = client.get(&release.asset.url);
    if resumed_from > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={resumed_from}-"));
    }
    let mut response = request.send().await.context("下载更新归档失败")?;
    let status = response.status();

    let (mut file, received, total) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let file = OpenOptions::new()
            .append(true)
            .open(&part)
            .with_context(|| format!("续写更新归档失败: {}", part.display()))?;
        let total = response
            .content_length()
            .map(|length| length.saturating_add(resumed_from));
        info!(resumed_from, total = ?total, "继续未完成的下载");
        (file, resumed_from, total)
    } else if status.is_success() {
        if resumed_from > 0 {
            debug!("服务器不支持断点续传, 重新下载");
        }
        let file = File::create(&part)
            .with_context(|| format!("创建更新归档失败: {}", part.display()))?;
        (file, 0, response.content_length())
    } else {
        bail!("下载更新归档失败: HTTP {status}");
    };

    progress(received, total);
    let mut received = received;
    while let Some(chunk) = response.chunk().await.context("读取更新归档数据失败")? {
        file.write_all(&chunk).context("写入更新归档失败")?;
        received = received.saturating_add(chunk.len() as u64);
        progress(received, total);
    }
    file.flush().context("刷新更新归档失败")?;
    drop(file);

    let digest = sha256_file(&part)?;
    if !digest.eq_ignore_ascii_case(&expected) {
        let _ = fs::remove_file(&part);
        bail!("更新归档 SHA256 校验失败, 已删除临时文件");
    }

    let archive = update_dir.join(&release.asset.name);
    fs::rename(&part, &archive)
        .with_context(|| format!("保存更新归档失败: {}", archive.display()))?;

    let pending = PendingUpdate {
        tag: release.tag.clone(),
        version: release.version.clone(),
        asset: release.asset.name.clone(),
        notes: release.notes.clone(),
        size: received,
    };
    fs::write(
        pending_path(update_dir),
        serde_json::to_vec_pretty(&pending).context("序列化更新记录失败")?,
    )
    .context("写入更新记录失败")?;
    info!(asset = %pending.asset, size = received, "更新包下载完成, 等待用户重启");

    Ok(pending)
}

/// 从 SHA256SUMS 内容中取出指定文件的摘要.
///
/// 容忍 `*` 二进制标记与 CRLF 行尾, 缺少对应行时报错.
pub fn parse_checksums(body: &str, file_name: &str) -> Result<String> {
    for line in body.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let Some((digest, name)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let name = name.trim().trim_start_matches('*');
        if name == file_name {
            if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
                bail!("SHA256SUMS 中 {file_name} 的摘要格式不正确");
            }
            return Ok(digest.to_owned());
        }
    }
    bail!("SHA256SUMS 中缺少 {file_name} 的摘要")
}

/// 计算文件摘要, 续传后的文件统一整文件计算.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("打开文件失败: {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).context("计算 SHA256 失败")?;
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn expected_digest(update_dir: &Path, file_name: &str) -> Result<String> {
    let body = fs::read_to_string(update_dir.join(CHECKSUMS_FILE)).context("读取本地 SHA256SUMS 失败")?;
    parse_checksums(&body, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_parsing_tolerates_markers() {
        let body = "abc123  other.zip\r\n\
                    d2a1b0c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f  *stcjudge-gui-1.0.0-linux-x86_64.tar.gz\n";
        let digest =
            parse_checksums(body, "stcjudge-gui-1.0.0-linux-x86_64.tar.gz").expect("parse digest");
        assert_eq!(
            digest,
            "d2a1b0c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f"
        );
        assert!(parse_checksums(body, "missing.zip").is_err());
    }

    #[test]
    fn sha256_matches_known_value() {
        let dir = std::env::temp_dir().join("stcjudge-sha256");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("payload.txt");
        std::fs::write(&path, b"abc").expect("write payload");
        assert_eq!(
            sha256_file(&path).expect("hash payload"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
