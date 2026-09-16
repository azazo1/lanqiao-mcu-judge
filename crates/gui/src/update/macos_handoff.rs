//! macOS 的更新交接.
//!
//! 进程存活时 LaunchServices 会占用已安装的 `.app`, 无法在进程内替换自己, 因此在用户点击
//! "重启并更新" 之后生成一个脱离进程的替换脚本, 由脚本等待旧进程退出, 挂载 dmg, 替换 bundle
//! 并重新拉起应用. 便携运行 (直接跑二进制) 时没有可替换的 bundle, 退回打开 dmg 的手动引导.

use std::{
    fs::{self, OpenOptions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use tracing::{debug, info};

/// 替换脚本文件名.
pub const SCRIPT_FILE: &str = "apply-update.sh";
/// 替换脚本日志文件名.
pub const LOG_FILE: &str = "apply-update.log";
/// 替换结果文件名, 由新进程启动时读取并展示.
pub const RESULT_FILE: &str = "apply-update-result.txt";

/// 等待旧进程退出的最长时间, 超过则放弃替换.
const OLD_PROCESS_TIMEOUT_SECS: u64 = 60;

/// bundle 以及由它推导出的同级暂存与备份路径.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundlePaths {
    pub bundle: PathBuf,
    pub staging: PathBuf,
    pub backup: PathBuf,
}

impl BundlePaths {
    /// 由当前可执行文件推导; 不在 `<bundle>.app/Contents/MacOS/` 结构内时返回 `None`.
    pub fn from_executable(exe: &Path) -> Option<Self> {
        let macos_dir = exe.parent()?;
        if macos_dir.file_name()? != "MacOS" {
            return None;
        }
        let contents = macos_dir.parent()?;
        if contents.file_name()? != "Contents" {
            return None;
        }
        let bundle = contents.parent()?;
        if bundle.extension()? != "app" {
            return None;
        }
        Some(Self::for_bundle(bundle))
    }

    /// 由 bundle 推导同级暂存与备份路径.
    ///
    /// 暂存与备份都必须是 bundle 所在目录下的同级条目, 不能拿 bundle 的父目录再向上取一层.
    pub fn for_bundle(bundle: &Path) -> Self {
        let parent = bundle.parent().unwrap_or_else(|| Path::new("."));
        let name = bundle
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "App.app".to_owned());
        Self {
            bundle: bundle.to_path_buf(),
            staging: parent.join(format!(".{name}.update-staging")),
            backup: parent.join(format!("{name}.old")),
        }
    }

    /// 暂存与备份是否确实与 bundle 同级.
    pub fn is_valid(&self) -> bool {
        self.staging.parent() == self.bundle.parent()
            && self.backup.parent() == self.bundle.parent()
            && self.staging != self.bundle
            && self.backup != self.bundle
    }
}

/// 生成替换脚本内容; 路径注入前做 shell 单引号转义.
pub fn build_script(
    paths: &BundlePaths,
    dmg: &Path,
    update_dir: &Path,
    old_pid: u32,
) -> Result<String> {
    if !paths.is_valid() {
        bail!("bundle 暂存或备份路径不在 bundle 同级, 拒绝执行替换");
    }

    let log = update_dir.join(LOG_FILE);
    let result = update_dir.join(RESULT_FILE);
    let mount = update_dir.join(format!("mount-{old_pid}"));

    Ok(format!(
        r##"#!/bin/bash
# 由 stcjudge GUI 生成的替换脚本, 等待旧进程退出后替换应用并重新拉起.
set -u
trap '' HUP
export PATH="/usr/bin:/bin:/usr/sbin:/sbin"

LOG={log}
RESULT={result}
BUNDLE={bundle}
STAGING={staging}
BACKUP={backup}
DMG={dmg}
MOUNT={mount}
OLD_PID={old_pid}
TIMEOUT={timeout}

exec >>"$LOG" 2>&1
echo "[$(date '+%F %T')] 开始替换: $BUNDLE"

renamed_old=0
mounted=0

detach() {{
    if [ "$mounted" = "1" ]; then
        hdiutil detach "$MOUNT" >/dev/null 2>&1
        mounted=0
    fi
}}

fail() {{
    echo "[$(date '+%F %T')] 替换失败: $1"
    detach
    if [ "$renamed_old" = "1" ] && [ -d "$BACKUP" ]; then
        rm -rf "$BUNDLE"
        mv "$BACKUP" "$BUNDLE" && echo "已回滚到旧版本"
    fi
    printf '%s\n' "$1" > "$RESULT"
    if [ -d "$BUNDLE" ]; then
        open "$BUNDLE" >/dev/null 2>&1
    fi
    exit 1
}}

# 破坏性操作之前先自检路径形状
[ "$(dirname "$STAGING")" = "$(dirname "$BUNDLE")" ] || fail "暂存路径不在应用同级"
[ "$(dirname "$BACKUP")" = "$(dirname "$BUNDLE")" ] || fail "备份路径不在应用同级"
[ -d "$BUNDLE" ] || fail "目标应用不存在: $BUNDLE"
[ -f "$DMG" ] || fail "更新镜像不存在: $DMG"
[ -w "$(dirname "$BUNDLE")" ] || fail "目标目录不可写, 请手动拖拽安装"

waited=0
while kill -0 "$OLD_PID" 2>/dev/null; do
    if [ "$waited" -ge "$TIMEOUT" ]; then
        echo "旧进程未在 $TIMEOUT 秒内退出"
        printf '%s\n' "旧进程未按时退出, 应用仍在运行, 请稍后重新发起更新" > "$RESULT"
        exit 1
    fi
    sleep 1
    waited=$((waited + 1))
done
echo "旧进程已退出, 用时 ${{waited}}s"

mkdir -p "$MOUNT" || fail "创建挂载点失败"
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" || fail "挂载更新镜像失败"
mounted=1

APP="$(find "$MOUNT" -maxdepth 1 -name '*.app' -print -quit)"
if [ -z "$APP" ]; then
    fail "更新镜像中没有找到应用包"
fi

rm -rf "$STAGING"
ditto "$APP" "$STAGING" || fail "复制新版本失败"
xattr -dr com.apple.quarantine "$STAGING" >/dev/null 2>&1
detach

mv "$BUNDLE" "$BACKUP" || fail "备份当前版本失败"
renamed_old=1
mv "$STAGING" "$BUNDLE" || fail "新版本就位失败"

rm -rf "$BACKUP"
rm -rf "$MOUNT"
rm -f "$RESULT"
echo "[$(date '+%F %T')] 替换完成"
open "$BUNDLE" >/dev/null 2>&1
"##,
        log = sh_quote(&log),
        result = sh_quote(&result),
        bundle = sh_quote(&paths.bundle),
        staging = sh_quote(&paths.staging),
        backup = sh_quote(&paths.backup),
        dmg = sh_quote(dmg),
        mount = sh_quote(&mount),
        old_pid = old_pid,
        timeout = OLD_PROCESS_TIMEOUT_SECS,
    ))
}

/// 把替换工作交给脱离进程的脚本; 便携运行时返回 `None`.
pub fn spawn_apply(dmg: &Path, update_dir: &Path) -> Result<Option<BundlePaths>> {
    let exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    let Some(paths) = BundlePaths::from_executable(&exe) else {
        return Ok(None);
    };

    fs::create_dir_all(update_dir)
        .with_context(|| format!("创建更新目录失败: {}", update_dir.display()))?;
    let script_path = update_dir.join(SCRIPT_FILE);
    let script = build_script(&paths, dmg, update_dir, std::process::id())?;
    fs::write(&script_path, script)
        .with_context(|| format!("写入替换脚本失败: {}", script_path.display()))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("设置替换脚本权限失败: {}", script_path.display()))?;

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(update_dir.join(LOG_FILE))
        .context("打开替换日志失败")?;
    let log_err = log.try_clone().context("复制替换日志句柄失败")?;
    let child = Command::new("/bin/bash")
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .context("启动替换脚本失败")?;

    info!(pid = child.id(), script = %script_path.display(), "已把替换工作交给替换脚本");
    Ok(Some(paths))
}

/// 启动时清理上次替换遗留的脚本, 挂载点与 bundle 备份, 保留日志.
pub fn cleanup_on_start(update_dir: &Path) {
    let _ = fs::remove_file(update_dir.join(SCRIPT_FILE));

    if let Ok(entries) = fs::read_dir(update_dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("mount-") {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(paths) = BundlePaths::from_executable(&exe) else {
        return;
    };
    if paths.backup.exists() {
        match fs::remove_dir_all(&paths.backup) {
            Ok(()) => debug!(path = %paths.backup.display(), "已清理上次替换遗留的备份"),
            Err(err) => debug!(path = %paths.backup.display(), error = ?err, "暂无法清理 bundle 备份"),
        }
    }
}

/// 读取并删除上次替换的结果文件, 供新进程展示失败原因.
pub fn take_result(update_dir: &Path) -> Option<String> {
    let path = update_dir.join(RESULT_FILE);
    let message = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    let message = message.trim().to_owned();
    (!message.is_empty()).then_some(message)
}

/// shell 单引号转义, 避免带空格或引号的路径破坏脚本.
fn sh_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("'{}'", raw.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_paths_stay_siblings() {
        let exe = Path::new("/Applications/Pinote.app/Contents/MacOS/pinote");
        let paths = BundlePaths::from_executable(exe).expect("bundle paths");

        assert_eq!(paths.bundle, Path::new("/Applications/Pinote.app"));
        assert_eq!(paths.staging, Path::new("/Applications/.Pinote.app.update-staging"));
        assert_eq!(paths.backup, Path::new("/Applications/Pinote.app.old"));
        assert!(paths.is_valid());
    }

    #[test]
    fn portable_binary_has_no_bundle() {
        assert!(BundlePaths::from_executable(Path::new("/Users/tester/pinote")).is_none());
        assert!(BundlePaths::from_executable(Path::new("/tmp/x/Contents/pinote")).is_none());
    }

    #[test]
    fn script_rejects_paths_outside_bundle_parent() {
        let bundle = PathBuf::from("/Applications/Pinote.app");
        let paths = BundlePaths {
            bundle: bundle.clone(),
            staging: PathBuf::from("/tmp/staging"),
            backup: bundle.with_file_name("Pinote.app.old"),
        };
        assert!(!paths.is_valid());
        assert!(
            build_script(&paths, Path::new("/tmp/update.dmg"), Path::new("/tmp"), 1).is_err()
        );
    }

    #[test]
    fn script_quotes_paths_with_spaces_and_quotes() {
        let bundle = PathBuf::from("/Applications/My App's.app");
        let paths = BundlePaths::for_bundle(&bundle);
        let script = build_script(
            &paths,
            Path::new("/Users/tester/Downloads/update.dmg"),
            Path::new("/Users/tester/Library/Application Support/stcjudge-gui/update"),
            4242,
        )
        .expect("build script");

        assert!(script.contains(r"BUNDLE='/Applications/My App'\''s.app'"), "{script}");
        assert!(script.contains("OLD_PID=4242"));
        assert!(script.contains(r"STAGING='/Applications/.My App'\''s.app.update-staging'"));
    }
}
