//! 设置文件的结构版本迁移.
//!
//! 每次结构变化都要显式提升 [`CURRENT_VERSION`], 并在这里补一段从旧版本升级的代码,
//! 而不是依赖字段默认值做隐式兼容.

use anyhow::{Result, bail};
use serde_json::Value;

/// 当前支持的设置文件版本.
pub const CURRENT_VERSION: u32 = 1;

/// 把任意历史版本的设置升级到 [`CURRENT_VERSION`].
pub fn migrate(value: &mut Value) -> Result<u32> {
    let mut version = read_version(value);

    while version < CURRENT_VERSION {
        match version {
            // 0 表示早期没有 version 字段的文件, 其字段集合与 v1 一致.
            0 => version = 1,
            other => bail!("设置文件版本 {other} 缺少迁移步骤"),
        }
    }

    if version > CURRENT_VERSION {
        bail!("设置文件版本 {version} 高于当前支持的 {CURRENT_VERSION}");
    }

    if let Some(object) = value.as_object_mut() {
        object.insert("version".to_owned(), Value::from(CURRENT_VERSION));
    }

    Ok(CURRENT_VERSION)
}

fn read_version(value: &Value) -> u32 {
    value
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versionless_file_migrates_to_current() {
        let mut value = json!({"update": {"auto_check": false}});
        assert_eq!(migrate(&mut value).expect("migrate"), CURRENT_VERSION);
        assert_eq!(value["version"], json!(CURRENT_VERSION));
    }

    #[test]
    fn future_version_is_rejected() {
        let mut value = json!({"version": CURRENT_VERSION + 1});
        assert!(migrate(&mut value).is_err());
    }
}
