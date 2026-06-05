key_mode(KEYBOARD);

fn boot() {
    reset(POWER_RESET);
    run_ms(240);
}

fn tap_once(key) {
    tap_key(key, 80);
}

fn assert_visible_digit(digit, timeout_ns, label) {
    let target = "seg.d" + digit + ".visible";
    let dt = run_to_state(target, true, timeout_ns);
    assert_in(dt, 0..=timeout_ns, label + " 可见相位");
}

fn assert_time_text(text, hour, minute, second, label) {
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + " 时间格式");
    assert_eq(parse_int(text[2..4]), hour, label + " 小时");
    assert_eq(text[4..5], ".", label + " 第1个分隔点");
    assert_eq(parse_int(text[5..7]), minute, label + " 分钟");
    assert_eq(text[7..8], ".", label + " 第2个分隔点");
    assert_eq(parse_int(text[8..10]), second, label + " 秒");
}

fn assert_time_screen(hour, minute, second, label) {
    assert_visible_digit(8, 300_000_000, label + " 采样");
    let text = display_text();
    assert_time_text(text, hour, minute, second, label);
}

fn assert_mode_switch(hour24, expected12, minute, second, label) {
    boot();
    set_rtc(hour24, minute, second);
    run_ms(120);
    assert_time_screen(hour24, minute, second, label + " 初始 24h");
    // S12: 从 24h 切到 12h.
    tap_once(S12);
    assert_time_screen(expected12, minute, second, label + " 切到 12h");
    // S12: 再从 12h 切回 24h.
    tap_once(S12);
    assert_time_screen(hour24, minute, second, label + " 切回 24h");
}

// 这 4 组样例覆盖午夜, 正午, 下午首小时, 以及 24h 上边界.
assert_mode_switch(0, 12, 5, 6, "00 点");
assert_mode_switch(12, 12, 34, 56, "12 点");
assert_mode_switch(13, 1, 59, 58, "13 点");
assert_mode_switch(23, 11, 45, 59, "23 点");
