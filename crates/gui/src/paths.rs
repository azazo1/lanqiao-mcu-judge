//! 数据目录, 日志文件与更新目录的位置解析.
//!
//! 正式构建与 `just fake-dist` 产出的测试构建使用彼此隔离的数据目录, 调试实例也可以通过
//! 环境变量指向项目内的目录, 因此三类实例不会互相干扰.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::build_info;

/// 覆盖数据目录的环境变量.
pub const DATA_DIR_ENV: &str = "STCJUDGE_GUI_DATA_DIR";
/// 覆盖主日志文件路径的环境变量.
pub const LOG_FILE_ENV: &str = "STCJUDGE_GUI_LOG_FILE";

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 应用数据目录, 不存在时创建.
pub fn data_dir() -> &'static Path {
    DATA_DIR.get_or_init(|| {
        let dir = resolve_data_dir();
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("创建数据目录失败 {}: {err}", dir.display());
        }
        dir
    })
}

fn resolve_data_dir() -> PathBuf {
    if let Some(dir) = env_dir(DATA_DIR_ENV) {
        return dir;
    }
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(data_dir_name())
}

fn data_dir_name() -> String {
    if build_info::is_fake_build() {
        format!("{}-fake", build_info::APP_NAME)
    } else {
        build_info::APP_NAME.to_owned()
    }
}

fn env_dir(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

/// 主日志文件路径.
pub fn log_file() -> PathBuf {
    env_dir(LOG_FILE_ENV).unwrap_or_else(|| data_dir().join("logs").join("app.log"))
}

/// 日志轮转后的留存目录.
pub fn log_dir() -> PathBuf {
    log_file()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| data_dir().to_path_buf())
}

/// 设置文件路径.
pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

/// 更新归档, 校验和与临时文件的存放目录.
pub fn update_dir() -> PathBuf {
    data_dir().join("update")
}

/// 单实例锁文件路径, 与数据目录绑定.
pub fn instance_lock_file() -> PathBuf {
    data_dir().join("instance.lock")
}

/// 记录当前主实例监听端口的文件.
pub fn instance_port_file() -> PathBuf {
    data_dir().join("instance.port")
}
