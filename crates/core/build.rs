//! 发布构建的版本注入.
//!
//! 只有显式设置 `PROJECT_BUILD_VERSION` 时才把版本号编进二进制, 日常开发构建保持 `dev-build`.
//! 构建脚本自身不读取 `.git`, 因此普通提交不会让增量编译缓存失效, `target` 也不会持续膨胀.

fn main() {
    println!("cargo:rerun-if-env-changed=PROJECT_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=PROJECT_DIST_FAKE");

    if let Ok(version) = std::env::var("PROJECT_BUILD_VERSION") {
        let version = version.trim();
        if !version.is_empty() {
            println!("cargo:rustc-env=STCJUDGE_BUILD_VERSION={version}");
        }
    }

    if std::env::var("PROJECT_DIST_FAKE").is_ok_and(|value| !value.is_empty() && value != "0") {
        println!("cargo:rustc-env=STCJUDGE_FAKE_BUILD=1");
    }
}
