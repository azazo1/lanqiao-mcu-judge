key_mode(KEYBOARD);

fn boot() {
    reset(POWER_RESET);
    run_ms(240);
}

fn tap_once(key) {
    tap_key(key, 80);
}

fn two_digits(value) {
    if value < 10 {
        return "0" + value;
    }
    return "" + value;
}

fn wait_seg_change_and_settle(timeout_ns, label) {
    let event = run_to_event("seg.change", timeout_ns);
    return event.elapsed_ns;
}

fn time_text(hour, minute, second) {
    return "  " + two_digits(hour) + "." + two_digits(minute) + "." + two_digits(second);
}

fn assert_time_text(text, hour, minute, second, label) {
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + ": 时间格式错误");
    assert_eq(parse_int(text[2..4]), hour, label + ": 小时显示错误");
    assert_eq(text[4..5], ".", label + ": 第1个分隔点显示错误");
    assert_eq(parse_int(text[5..7]), minute, label + ": 分钟显示错误");
    assert_eq(text[7..8], ".", label + ": 第2个分隔点显示错误");
    assert_eq(parse_int(text[8..10]), second, label + ": 秒显示错误");
}

fn assert_time_screen(hour, minute, second, label) {
    let expected = time_text(hour, minute, second);
    let current = display_text();
    let text = if current != expected {
        wait_seg_change_and_settle(300_000_000, label + ": 显示同步");
        display_text(20)
    } else {
        current
    };
    assert_time_text(text, hour, minute, second, label);
}

fn assert_mode_switch(hour24, expected12, minute, second, label) {
    boot();
    set_rtc(hour24, minute, second);
    run_ms(120);
    assert_time_screen(hour24, minute, second, label + ": 初始 24h");
    // S12: 从 24h 切到 12h.
    tap_once(S12);
    assert_time_screen(expected12, minute, second, label + ": 切到 12h");
    // S12: 再从 12h 切回 24h.
    tap_once(S12);
    assert_time_screen(hour24, minute, second, label + ": 切回 24h");
}

// 这 4 组样例覆盖午夜, 正午, 下午首小时, 以及 24h 上边界.
assert_mode_switch(0, 12, 5, 6, "00 点");
assert_mode_switch(12, 12, 34, 56, "12 点");
assert_mode_switch(13, 1, 59, 58, "13 点");
assert_mode_switch(23, 11, 45, 59, "23 点");
