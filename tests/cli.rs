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

fn ds18b20_expected_milli_celsius(temp_c: f64, resolution_level: u8) -> i64 {
    let raw_12bit = (temp_c * 16.0).round() as i64;
    let raw = match resolution_level {
        0 => raw_12bit & !0x7,
        1 => raw_12bit & !0x3,
        2 => raw_12bit & !0x1,
        _ => raw_12bit,
    };
    ((raw as f64) * 0.0625 * 1000.0) as i64
}

fn ds18b20_expected_display_celsius(temp_c: f64, resolution_level: u8) -> f64 {
    ds18b20_expected_milli_celsius(temp_c, resolution_level) as f64 / 1000.0
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
fn rhai_ckpt_prints_table_and_keeps_failing_exit_code() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "ckpt(1, \"失败项\", \"应当失败\", || { assert_eq(1, 2, \"bad\"); });\nckpt(2, \"通过项\", \"应当通过\", || { \"实际通过\" });\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args(["run", "--script", script_path.to_str().expect("script path")])
        .output()
        .expect("run cli");

    let _ = std::fs::remove_file(&script_path);

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("| 序号 "), "{stdout}");
    assert!(stdout.contains("| --- | --- | --- | --- | --- |"), "{stdout}");
    assert!(stdout.contains("| 1 | 失败项 |"), "{stdout}");
    assert!(stdout.contains("| 2 | 通过项 |"), "{stdout}");
    assert!(stdout.contains("❌ 失败"), "{stdout}");
    assert!(stdout.contains("✅ 通过"), "{stdout}");
    assert!(stdout.contains("bad: 期望 2 , 实际 1"), "{stdout}");
    assert!(stdout.contains("实际通过"), "{stdout}");
    assert!(!stdout.contains("in closure call"), "{stdout}");
    assert!(!stdout.contains("in call to function"), "{stdout}");
    assert!(stderr.contains("ckpt 失败: 1/2"), "{stderr}");
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
        "let s = \"Hello, World!\";\nassert_eq(s[0..5], \"Hello\", \"slice\");\nassert_eq(parse_int(\" 20\"), 20, \"parse_int\");\nassert_eq(1.5, 1.5, \"float_eq\");\nassert_eq(regex_is_match(\"23-59-50\", \"^\\\\d{2}-\\\\d{2}-\\\\d{2}$\"), true, \"regex\");\nassert_eq(parse_float(\"123.45\"), 123.45, \"parse_float\");\nassert_in(2, 1..3, \"int_range\");\nassert_in(123.45, 123..124, \"float_range\");\n",
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
        "run_ms(220);\nassert_eq(parse_int(display_text(30)[0..3]), 0, \"display\");\nlet stats = watch_led_stats(L1, 40);\nassert_in(stats.pwm_frequency_hz, 950..=1050, \"freq\");\nassert_in(stats.duty_percent, 8..=12, \"duty\");\n",
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
        "key_mode(BUTTON);\nrun_ms(400);\nassert_eq(da_value(), 127, \"boot da\");\n",
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
        "key_mode(BUTTON);\nset_voltage(AIN3, 1.0);\nset_voltage(AIN1, 4.0);\nrun_ms(500);\nlet text = display_text(30);\nassert_in(parse_int(text[0..3]), 50..=52, \"ain3\");\nassert_in(parse_int(text[4..7]), 203..=205, \"ain1\");\nset_voltage(RB2, 4.0);\nset_voltage(RD1, 1.0);\nrun_ms(500);\nlet text2 = display_text(30);\nassert_in(parse_int(text2[0..3]), 203..=205, \"rb2\");\nassert_in(parse_int(text2[4..7]), 50..=52, \"rd1\");\n",
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
fn rhai_ds18b20_resolution_levels_follow_float_temperature() {
    let level0 = ds18b20_expected_display_celsius(25.9375, 0);
    let level1 = ds18b20_expected_display_celsius(25.9375, 1);
    let level2 = ds18b20_expected_display_celsius(25.9375, 2);
    let level3 = ds18b20_expected_display_celsius(25.9375, 3);
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        format!(
            "run_ms(700);\nrun_ms(30);\nassert_eq(display_number(1, 6), 0.000, \"boot temp\");\nassert_eq(display_number(8, 8), 0, \"boot level\");\nset_temperature_c(25.9375);\nrun_ms(700);\nrun_ms(30);\nassert_eq(display_number(1, 6), {level0:.3}, \"level0 temp\");\nassert_eq(display_number(8, 8), 0, \"level0\");\ntap_key(S5, 80);\nrun_ms(400);\nrun_ms(30);\nassert_eq(display_number(1, 6), {level1:.3}, \"level1 temp\");\nassert_eq(display_number(8, 8), 1, \"level1\");\ntap_key(S5, 80);\nrun_ms(400);\nrun_ms(30);\nassert_eq(display_number(1, 6), {level2:.3}, \"level2 temp\");\nassert_eq(display_number(8, 8), 2, \"level2\");\ntap_key(S5, 80);\nrun_ms(400);\nrun_ms(30);\nassert_eq(display_number(1, 6), {level3:.3}, \"level3 temp\");\nassert_eq(display_number(8, 8), 3, \"level3\");\n"
        ),
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/ds18b20/prj/Objects/ds18b20.hex")
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
fn rhai_ds18b20_temperature_range_handles_negative_and_high_values() {
    let script_path = temp_script_path();
    std::fs::write(
        &script_path,
        "set_temperature_c(-25);\nrun_ms(700);\nassert_eq(display_number(1, 6), -25.000, \"minus25\");\nset_temperature_c(100);\nrun_ms(700);\nassert_eq(display_number(1, 5), 100.0, \"plus100\");\n",
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_stcjudge"))
        .args([
            "run",
            "--hex",
            sample_path("sample/ds18b20/prj/Objects/ds18b20.hex")
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
