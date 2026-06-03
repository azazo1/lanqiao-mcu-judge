use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
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

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
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

#[test]
fn cli_accepts_stdin_script_and_builtin_constants() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/key_seg/prj/Objects/key_seg.hex")
                .to_str()
                .expect("hex path"),
            "--stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli");

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        stdin
            .write_all(b"run_ms(220);\nset_key(S4, true);\nrun_ms(220);\nprint(display_text());\n")
            .expect("write stdin script");
    }

    let output = child.wait_with_output().expect("wait cli");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "       1"),
        "stdout: {stdout}"
    );
}

#[test]
fn debug_tracing_keeps_script_execution_working() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "run_ms(220);\nset_key(S4, true);\nrun_ms(220);\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .env("RUST_LOG", "debug")
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
        .expect("run cli with debug tracing");

    let _ = std::fs::remove_file(&script_path);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhai_regex_and_native_string_slice_work() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "let s = \"Hello, World!\";\nassert_eq_str(s[0..5], \"Hello\", \"slice\");\nassert(regex_is_match(\"23-59-50\", \"^\\\\d{2}-\\\\d{2}-\\\\d{2}$\"), \"regex\");\nassert(parse_int(\" 20\") == 20, \"parse_int\");\nassert(parse_float(\"123.45\") > 123.4, \"parse_float\");\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
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
}

#[test]
fn rhai_led_pwm_watchers_work() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "run_ms(220);\nassert(parse_int(display_text(30)[0..3]) == 0, \"display\");\nlet stats = watch_led_stats(L1, 40);\nassert(stats.pwm_frequency_hz >= 950.0 && stats.pwm_frequency_hz <= 1050.0, \"freq\");\nassert(stats.duty_percent >= 8.0 && stats.duty_percent <= 12.0, \"duty\");\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/led_pwm/prj/Objects/led_pwm.hex")
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
}

#[test]
fn rhai_da_value_reports_pcf8591_output() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "key_mode(BUTTON);\nrun_ms(400);\nassert(da_value() == 127, \"boot da\");\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/ad_da/prj/Objects/ad_da.hex")
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
}

#[test]
fn rhai_voltage_aliases_drive_pcf8591_inputs() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "key_mode(BUTTON);\nset_voltage(AIN3, 1.0);\nset_voltage(AIN1, 4.0);\nrun_ms(500);\nlet text = display_text(30);\nassert(parse_int(text[0..3]) >= 50 && parse_int(text[0..3]) <= 52, \"ain3\");\nassert(parse_int(text[4..7]) >= 203 && parse_int(text[4..7]) <= 205, \"ain1\");\nset_voltage(RB2, 4.0);\nset_voltage(RD1, 1.0);\nrun_ms(500);\nlet text2 = display_text(30);\nassert(parse_int(text2[0..3]) >= 203 && parse_int(text2[0..3]) <= 205, \"rb2\");\nassert(parse_int(text2[4..7]) >= 50 && parse_int(text2[4..7]) <= 52, \"rd1\");\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/ad_da/prj/Objects/ad_da.hex")
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
}
