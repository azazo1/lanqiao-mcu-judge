//! 设置项持久化.

mod migration;

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::paths;

pub use migration::CURRENT_VERSION;

/// 应用设置.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// 设置文件结构版本, 用于后续自动迁移.
    pub version: u32,
    #[serde(default)]
    pub update: UpdateSettings,
}

/// 自动更新相关设置.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSettings {
    /// 逻辑键 `update.auto_check`, 控制启动后是否静默检查更新.
    #[serde(default = "default_auto_check")]
    pub auto_check: bool,
    /// 逻辑键 `update.skipped_version`, 记录被用户跳过的版本, 静默检查不再提示该版本.
    #[serde(default)]
    pub skipped_version: Option<String>,
}

fn default_auto_check() -> bool {
    true
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: default_auto_check(),
            skipped_version: None,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            update: UpdateSettings::default(),
        }
    }
}

impl Settings {
    /// 读取设置文件; 文件缺失时返回默认值, 内容损坏时记录警告后返回默认值.
    pub fn load() -> Self {
        Self::load_from(&paths::settings_file())
    }

    pub fn load_from(path: &Path) -> Self {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                debug!(path = %path.display(), "设置文件不存在, 使用默认设置");
                return Self::default();
            }
            Err(err) => {
                warn!(path = %path.display(), error = ?err, "读取设置文件失败, 使用默认设置");
                return Self::default();
            }
        };

        let mut value: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(err) => {
                warn!(path = %path.display(), error = ?err, "设置文件不是合法 JSON, 使用默认设置");
                return Self::default();
            }
        };

        let migrated = match migration::migrate(&mut value) {
            Ok(version) => version,
            Err(err) => {
                warn!(path = %path.display(), error = ?err, "设置文件版本不受支持, 使用默认设置");
                return Self::default();
            }
        };

        match serde_json::from_value::<Settings>(value) {
            Ok(mut settings) => {
                settings.version = migrated;
                debug!(path = %path.display(), version = settings.version, "已读取设置");
                settings
            }
            Err(err) => {
                warn!(path = %path.display(), error = ?err, "设置文件字段不合法, 使用默认设置");
                Self::default()
            }
        }
    }

    /// 写回设置文件, 采用先写临时文件再改名的方式, 避免写到一半损坏原文件.
    pub fn save(&self) -> Result<()> {
        self.save_to(&paths::settings_file())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建设置目录失败: {}", parent.display()))?;
        }
        let body = serde_json::to_string_pretty(self).context("序列化设置失败")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, body.as_bytes())
            .with_context(|| format!("写入设置临时文件失败: {}", temp.display()))?;
        std::fs::rename(&temp, path)
            .with_context(|| format!("更新设置文件失败: {}", path.display()))?;
        debug!(path = %path.display(), "设置已保存");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = std::env::temp_dir().join("stcjudge-settings-missing");
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);
        let settings = Settings::load_from(&path);
        assert_eq!(settings, Settings::default());
        assert!(settings.update.auto_check);
    }

    #[test]
    fn legacy_file_without_version_is_migrated() {
        let dir = std::env::temp_dir().join("stcjudge-settings-legacy");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("settings.json");
        std::fs::write(&path, br#"{"update":{"auto_check":false}}"#).expect("write legacy file");

        let settings = Settings::load_from(&path);

        assert_eq!(settings.version, CURRENT_VERSION);
        assert!(!settings.update.auto_check);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join("stcjudge-settings-round-trip");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("settings.json");

        let settings = Settings {
            version: CURRENT_VERSION,
            update: UpdateSettings {
                auto_check: false,
                skipped_version: Some("1.2.3".to_owned()),
            },
        };
        settings.save_to(&path).expect("save settings");

        assert_eq!(Settings::load_from(&path), settings);
    }
}
