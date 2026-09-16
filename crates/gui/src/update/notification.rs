//! 系统通知.

use tracing::debug;

use crate::build_info;

/// 静默检查发现新版本时发系统通知, 引导用户打开主界面查看.
pub fn notify_update_available(version: &str) {
    let result = notify_rust::Notification::new()
        .summary(&format!("{} 有新版本 {version}", build_info::APP_TITLE))
        .body("打开 stcjudge GUI, 点击状态栏中的版本号即可查看更新")
        .show();

    match result {
        Ok(_) => debug!(version, "已发送新版本通知"),
        Err(err) => debug!(version, error = ?err, "发送新版本通知失败"),
    }
}
