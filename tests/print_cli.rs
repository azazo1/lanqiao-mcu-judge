use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn sample_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn temp_script_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    path.push(format!(
        "lanqiao_mcu_judge_print_{}_{}.rhai",
        std::process::id(),
        nonce
    ));
    path
}

#[test]
fn rhai_print_writes_to_stdout() {
    let script_path = temp_script_path();
    std::fs::write(&script_path, "run_ms(220);\nprint(display_text());\n").expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_lanqiao-mcu-judge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/key_seg/prj/Objects/key_seg.hex")
                .to_str()
                .expect("hex path"),
            "--script",
            script_path.to_str().expect("script path"),
        ])
        .output()
        .expect("run cli");

    let _ = std::fs::remove_file(&script_path);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "       0"),
        "stdout: {stdout}"
    );
}
