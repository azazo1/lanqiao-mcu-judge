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

fn time_text(hour, minute, second) {
    return "  " + two_digits(hour) + "." + two_digits(minute) + "." + two_digits(second);
}

fn assert_visible_digit(digit, timeout_ns, label) {
    let target = "seg.d" + digit + ".visible";
    run_to_state(target, true, timeout_ns);
}

fn wait_seg_change_and_settle(timeout_ns, label) {
    let event = run_to_event("seg.change", timeout_ns);
    return event.elapsed_ns;
}

fn assert_setting_field_visible(start_digit, timeout_ns, label) {
    let end_digit = start_digit + 1;
    let target = "seg.d" + end_digit + ".visible";
    let dt_off = run_to_state(target, false, timeout_ns);
    let remain_ns = timeout_ns - dt_off;
    run_to_state(target, true, remain_ns);
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
    assert_eq(text, expected, label + ": 可见文本错误");
    assert_time_text(text, hour, minute, second, label);
}

fn assert_setting_time_screen(visible_digit, hour, minute, second, label) {
    let expected = time_text(hour, minute, second);
    let current = display_text();
    let text;
    if current != expected {
        let spent_ns = wait_seg_change_and_settle(1_200_000_000, label + ": 显示同步");
        let remain_ns = 1_200_000_000 - spent_ns;
        assert_visible_digit(visible_digit + 1, remain_ns, label + ": 可见相位");
        text = display_text(20);
    } else {
        assert_setting_field_visible(visible_digit, 1_200_000_000, label + ": 选中字段");
        text = display_text(20);
    }
    assert_eq(text, expected, label + ": 可见文本错误");
    assert_time_text(text, hour, minute, second, label);
}

fn assert_time_resume(hour, minute, old_second, new_second, min_elapsed_ns, max_elapsed_ns, label) {
    let event = run_to_event("seg.change", max_elapsed_ns);
    assert_in(event.elapsed_ns, min_elapsed_ns..=max_elapsed_ns, label + ": 恢复计时时长超出范围");
    let text = display_text(20);
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + ": 时间格式错误");
    assert_eq(parse_int(text[2..4]), hour, label + ": 小时显示错误");
    assert_eq(parse_int(text[5..7]), minute, label + ": 分钟显示错误");
    assert_eq(parse_int(text[8..10]), new_second, label + ": 恢复后的秒值错误");
    assert(new_second != old_second, label + ": 秒值恢复错误, old_second=" + old_second + ", new_second=" + new_second);
}

// 场景 1. 进入设置态后, 时间应冻结在进入时的快照.
boot();
set_rtc(1, 2, 58);
assert_time_screen(1, 2, 58, "设置前时间");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 1, 2, 58, "进入设置态后仍显示原时间");
run_ms(1800);
assert_setting_time_screen(3, 1, 2, 58, "设置态冻结显示");

// 场景 2. 小时字段上边界应从 23 回绕到 00.
boot();
set_rtc(23, 11, 22);
assert_time_screen(23, 11, 22, "小时上边界初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 23, 11, 22, "进入小时设置态");
// S9: 小时从 23 加到 00.
tap_once(S9);
assert_setting_time_screen(3, 0, 11, 22, "小时从 23 回绕到 00");
// S4: 退出设置态并保存.
tap_once(S4);
assert_time_screen(0, 11, 22, "保存小时回绕结果");

// 场景 3. 分钟字段上边界应从 59 回绕到 00.
boot();
set_rtc(11, 59, 22);
assert_time_screen(11, 59, 22, "分钟上边界初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 11, 59, 22, "进入时间设置态准备改分钟");
// S7: 切到分钟字段.
tap_once(S7);
assert_setting_time_screen(5, 11, 59, 22, "切到分钟字段");
// S9: 分钟从 59 加到 00.
tap_once(S9);
assert_setting_time_screen(5, 11, 0, 22, "分钟从 59 回绕到 00");
// S4: 退出设置态并保存.
tap_once(S4);
assert_time_screen(11, 0, 22, "保存分钟回绕结果");

// 场景 4. 秒字段上边界应从 59 回绕到 00.
boot();
set_rtc(11, 22, 59);
assert_time_screen(11, 22, 59, "秒上边界初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 11, 22, 59, "进入时间设置态准备改秒");
// S7: 切到分钟字段.
tap_once(S7);
assert_setting_time_screen(5, 11, 22, 59, "切到分钟字段准备继续");
// S7: 切到秒字段.
tap_once(S7);
assert_setting_time_screen(7, 11, 22, 59, "切到秒字段");
// S9: 秒从 59 加到 00.
tap_once(S9);
assert_setting_time_screen(7, 11, 22, 0, "秒从 59 回绕到 00");
// S4: 退出设置态并保存.
tap_once(S4);
assert_time_screen(11, 22, 0, "保存秒回绕结果");

// 场景 5. `S6` 应支持从小时字段反向回绕到秒字段.
boot();
set_rtc(11, 22, 0);
assert_time_screen(11, 22, 0, "反向选字段初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 11, 22, 0, "进入时间设置态准备反向选字段");
// S6: 从小时字段反向回绕到秒字段.
tap_once(S6);
assert_setting_time_screen(7, 11, 22, 0, "S6 反向回绕到秒字段");
// S8: 秒从 00 减到 59.
tap_once(S8);
assert_setting_time_screen(7, 11, 22, 59, "秒从 00 回绕到 59");
// S4: 退出设置态并保存.
tap_once(S4);
assert_time_screen(11, 22, 59, "保存秒下边界回绕结果");

// 场景 6. 小时字段下边界应从 00 回绕到 23.
boot();
set_rtc(0, 11, 22);
assert_time_screen(0, 11, 22, "小时下边界初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 0, 11, 22, "进入时间设置态准备改小时");
// S8: 小时从 00 减到 23.
tap_once(S8);
assert_setting_time_screen(3, 23, 11, 22, "小时从 00 回绕到 23");
// S4: 退出设置态并保存.
tap_once(S4);
assert_time_screen(23, 11, 22, "保存小时下边界回绕结果");

// 场景 7. 保存后应恢复计时.
boot();
set_rtc(1, 2, 58);
assert_time_screen(1, 2, 58, "恢复计时初始值");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 1, 2, 58, "进入设置态准备直接保存");
// S4: 不改值直接退出并保存.
tap_once(S4);
assert_time_screen(1, 2, 58, "直接保存后仍显示原时间");
assert_time_resume(1, 2, 58, 59, 500_000_000, 1_500_000_000, "退出设置态后恢复计时");
assert_time_screen(1, 2, 59, "退出设置态后恢复计时结果");
