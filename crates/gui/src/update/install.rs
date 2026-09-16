//! linux 与 windows 的就地更新.
//!
//! 这两平台上运行中的二进制只能改名不能覆盖删除, 因此先把当前程序改名为 `<exe>.old` 让位,
//! 再把新程序搬到原路径, 中途失败就回滚; 替换动作只在用户点击 "重启并更新" 后执行.

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use tracing::{debug, info};

use crate::instance::RELAUNCH_ENV;

/// 启动时清理上次更新遗留的备份文件; 文件被占用时留待下次.
pub fn cleanup_stale_backup() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let backup = backup_path(&exe);
    if backup.is_file() {
        match fs::remove_file(&backup) {
            Ok(()) => debug!(path = %backup.display(), "已清理上次更新遗留的备份"),
            Err(err) => debug!(path = %backup.display(), error = ?err, "暂无法清理更新备份"),
        }
    }
}

/// 解包归档, 替换当前可执行文件并拉起新进程, 返回新程序路径.
pub fn apply_archive(archive: &Path, work_dir: &Path, restart_args: &[String]) -> Result<PathBuf> {
    let exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    let program = exe
        .file_name()
        .and_then(|name| name.to_str())
        .context("当前可执行文件名不合法")?
        .to_owned();
    let new_exe = extract_program(archive, &program, &work_dir.join("extracted"))?;

    let backup = backup_path(&exe);
    let _ = fs::remove_file(&backup);
    fs::rename(&exe, &backup).with_context(|| {
        format!(
            "让位当前程序失败, 请检查写入权限: {}",
            exe.display()
        )
    })?;

    if let Err(err) = move_file(&new_exe, &exe) {
        let _ = fs::rename(&backup, &exe);
        return Err(err);
    }
    #[cfg(unix)]
    set_executable(&exe)?;

    match Command::new(&exe)
        .args(restart_args)
        .env(RELAUNCH_ENV, "1")
        .spawn()
    {
        Ok(child) => {
            info!(pid = child.id(), program = %exe.display(), "已拉起新版本进程");
            Ok(exe)
        }
        Err(err) => {
            let _ = fs::remove_file(&exe);
            let _ = fs::rename(&backup, &exe);
            Err(anyhow::Error::new(err).context("拉起新版本进程失败, 已回滚"))
        }
    }
}

/// 从归档中解出目标程序到 `dest_dir`.
pub fn extract_program(archive: &Path, program: &str, dest_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("创建解包目录失败: {}", dest_dir.display()))?;
    let target = dest_dir.join(program);
    let archive_name = archive
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let found = if archive_name.ends_with(".zip") {
        extract_from_zip(archive, program, &target)?
    } else if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
        extract_from_tar_gz(archive, program, &target)?
    } else {
        bail!("不支持的更新归档格式: {}", archive.display());
    };

    if !found {
        bail!("归档 {} 中没有找到 {program}", archive.display());
    }
    Ok(target)
}

fn extract_from_zip(archive: &Path, program: &str, target: &Path) -> Result<bool> {
    let file = File::open(archive).with_context(|| format!("打开归档失败: {}", archive.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).with_context(|| format!("读取归档失败: {}", archive.display()))?;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).context("读取归档条目失败")?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_owned();
        if Path::new(&entry_name).file_name().and_then(|name| name.to_str()) != Some(program) {
            continue;
        }
        let mut out = File::create(target)
            .with_context(|| format!("创建解包文件失败: {}", target.display()))?;
        io::copy(&mut entry, &mut out).context("解包归档条目失败")?;
        return Ok(true);
    }
    Ok(false)
}

fn extract_from_tar_gz(archive: &Path, program: &str, target: &Path) -> Result<bool> {
    let file = File::open(archive).with_context(|| format!("打开归档失败: {}", archive.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);

    for entry in tar.entries().context("读取归档条目失败")? {
        let mut entry = entry.context("读取归档条目失败")?;
        let path = entry.path().context("读取归档条目路径失败")?.into_owned();
        if path.file_name().and_then(|name| name.to_str()) != Some(program) {
            continue;
        }
        entry
            .unpack(target)
            .with_context(|| format!("解包文件失败: {}", target.display()))?;
        return Ok(true);
    }
    Ok(false)
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) => {
            debug!(error = ?err, "改名失败, 退回复制方式");
            fs::copy(from, to)
                .with_context(|| format!("复制新程序失败: {}", to.display()))?;
            let _ = fs::remove_file(from);
            Ok(())
        }
    }
}

fn backup_path(exe: &Path) -> PathBuf {
    let mut backup = exe.as_os_str().to_owned();
    backup.push(".old");
    PathBuf::from(backup)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("读取权限失败: {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("设置可执行权限失败: {}", path.display()))
}
