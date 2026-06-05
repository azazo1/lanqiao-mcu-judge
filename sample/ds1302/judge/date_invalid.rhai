key_mode(KEYBOARD);

fn boot() {
    reset(POWER_RESET);
    run_ms(240);
}

fn tap_once(key) {
    tap_key(key, 80);
}

fn tap_many(key, count) {
    let i = 0;
    while i < count {
        tap_once(key);
        i += 1;
    }
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

fn date_text(year, month, date, day) {
    return two_digits(year) + "." + two_digits(month) + "." + two_digits(date) + "." + two_digits(day);
}

fn assert_visible_digit(digit, timeout_ns, label) {
    let target = "seg.d" + digit + ".visible";
    let dt = run_to_state(target, true, timeout_ns);
    assert_in(dt, 0..=timeout_ns, label + " 可见相位");
}

fn wait_seg_change_and_settle(timeout_ns, label) {
    let event = run_to_event("seg.change", timeout_ns);
    assert_in(event.elapsed_ns, 0..=timeout_ns, label + " 首次段码变化时延");
    run_ms(20);
    return event.elapsed_ns + 20_000_000;
}

fn assert_setting_field_visible(start_digit, timeout_ns, label) {
    let end_digit = start_digit + 1;
    let target = "seg.d" + end_digit + ".visible";
    let dt_off = run_to_state(target, false, timeout_ns);
    assert_in(dt_off, 0..=timeout_ns, label + " 进入隐藏相位");
    let remain_ns = timeout_ns - dt_off;
    assert(remain_ns >= 0, label + " 重新显示预算应非负");
    let dt_on = run_to_state(target, true, remain_ns);
    assert_in(dt_on, 0..=remain_ns, label + " 重新显示");
    run_ms(20);
}

fn assert_time_text(text, hour, minute, second, label) {
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + " 时间格式");
    assert_eq(parse_int(text[2..4]), hour, label + " 小时");
    assert_eq(parse_int(text[5..7]), minute, label + " 分钟");
    assert_eq(parse_int(text[8..10]), second, label + " 秒");
}

fn assert_date_text(text, year, month, date, day, label) {
    assert_regex(text, "^\\d{2}\\.\\d{2}\\.\\d{2}\\.\\d{2}$", label + " 日期格式");
    assert_eq(parse_int(text[0..2]), year, label + " 年");
    assert_eq(parse_int(text[3..5]), month, label + " 月");
    assert_eq(parse_int(text[6..8]), date, label + " 日");
    assert_eq(parse_int(text[9..11]), day, label + " 周");
}

fn assert_time_screen(hour, minute, second, label) {
    let expected = time_text(hour, minute, second);
    let current = display_text();
    if current != expected {
        wait_seg_change_and_settle(300_000_000, label + " 显示同步");
    }
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_time_text(text, hour, minute, second, label);
}

fn wrap_day_seconds(total_seconds) {
    let day_seconds = 24 * 60 * 60;
    let wrapped = total_seconds % day_seconds;
    if wrapped < 0 {
        return wrapped + day_seconds;
    }
    return wrapped;
}

fn assert_time_keeps_running(base_hour, base_minute, base_second, start_ns, edited_hour, edited_minute, edited_second, label) {
    let current = display_text();
    if !regex_is_match(current, "^  \\d{2}\\.\\d{2}\\.\\d{2}$") {
        wait_seg_change_and_settle(300_000_000, label + " 显示同步");
    }
    let text = display_text();
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + " 时间格式");

    let actual_hour = parse_int(text[2..4]);
    let actual_minute = parse_int(text[5..7]);
    let actual_second = parse_int(text[8..10]);
    let actual_total = actual_hour * 3600 + actual_minute * 60 + actual_second;

    let elapsed_ns = sim_time_ns() - start_ns;
    let elapsed_seconds = elapsed_ns / 1_000_000_000;
    let base_total = base_hour * 3600 + base_minute * 60 + base_second;
    let expected_total = wrap_day_seconds(base_total + elapsed_seconds);
    let previous_total = wrap_day_seconds(expected_total - 1);
    let edited_total = edited_hour * 3600 + edited_minute * 60 + edited_second;

    assert(
        actual_total == expected_total || actual_total == previous_total,
        label + " 应保持原时间继续走时, 当前 " + text
    );
    assert(actual_total != edited_total, label + " 不应写入编辑后的时间, 当前 " + text);
}

fn assert_setting_time_screen(visible_digit, hour, minute, second, label) {
    let expected = time_text(hour, minute, second);
    let current = display_text();
    if current != expected {
        let spent_ns = wait_seg_change_and_settle(1_200_000_000, label + " 显示同步");
        let remain_ns = 1_200_000_000 - spent_ns;
        assert(remain_ns >= 0, label + " 可见相位预算应非负");
        assert_visible_digit(visible_digit + 1, remain_ns, label + " 可见相位");
        run_ms(20);
    } else {
        assert_setting_field_visible(visible_digit, 1_200_000_000, label + " 选中字段");
    }
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_time_text(text, hour, minute, second, label);
}

fn assert_date_screen(year, month, date, day, label) {
    let expected = date_text(year, month, date, day);
    let current = display_text();
    if current != expected {
        wait_seg_change_and_settle(300_000_000, label + " 显示同步");
    }
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_date_text(text, year, month, date, day, label);
}

fn assert_setting_date_screen(visible_digit, year, month, date, day, label) {
    let expected = date_text(year, month, date, day);
    let current = display_text();
    if current != expected {
        let spent_ns = wait_seg_change_and_settle(1_200_000_000, label + " 显示同步");
        let remain_ns = 1_200_000_000 - spent_ns;
        assert(remain_ns >= 0, label + " 可见相位预算应非负");
        assert_visible_digit(visible_digit + 1, remain_ns, label + " 可见相位");
        run_ms(20);
    } else {
        assert_setting_field_visible(visible_digit, 1_200_000_000, label + " 选中字段");
    }
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_date_text(text, year, month, date, day, label);
}

// 场景 1. 非闰年 2 月 29 日应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "非闰年场景切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
assert_setting_date_screen(1, 26, 6, 5, 5, "进入日期设置态");
// S8: 年份从 26 减到 25, 构造非闰年.
tap_once(S8);
assert_setting_date_screen(1, 25, 6, 5, 5, "年份减到非闰年 25");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 25, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
assert_setting_date_screen(3, 25, 4, 5, 5, "月份减到 04");
// S8 x2: 月份从 04 减到 02.
tap_many(S8, 2);
assert_setting_date_screen(3, 25, 2, 5, 5, "月份减到 02");
// S7: 切到日期字段.
tap_once(S7);
assert_setting_date_screen(5, 25, 2, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
assert_setting_date_screen(5, 25, 2, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
assert_setting_date_screen(5, 25, 2, 1, 5, "日期减到 01");
// S8: 日期从 01 减到 00.
tap_once(S8);
assert_setting_date_screen(5, 25, 2, 0, 5, "日期减到 00");
// S8: 日期从 00 回绕到 31.
tap_once(S8);
assert_setting_date_screen(5, 25, 2, 31, 5, "日期从 00 回绕到 31");
// S8: 日期从 31 减到 30.
tap_once(S8);
assert_setting_date_screen(5, 25, 2, 30, 5, "日期减到 30");
// S8: 日期从 30 减到 29.
tap_once(S8);
assert_setting_date_screen(5, 25, 2, 29, 5, "日期减到非法的 29");
// S7: 切到星期字段.
tap_once(S7);
assert_setting_date_screen(7, 25, 2, 29, 5, "切到星期字段");
// S9: 星期从 05 加到 06.
tap_once(S9);
assert_setting_date_screen(7, 25, 2, 29, 6, "星期加到 06");
// S4: 尝试保存非法日期.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "非闰年 2 月 29 日应拒绝写回");

// 场景 2. 4 月 31 日应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "4 月场景切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
assert_setting_date_screen(1, 26, 6, 5, 5, "进入 4 月设置态");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
assert_setting_date_screen(3, 26, 4, 5, 5, "月份减到 04");
// S7: 切到日期字段.
tap_once(S7);
assert_setting_date_screen(5, 26, 4, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
assert_setting_date_screen(5, 26, 4, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
assert_setting_date_screen(5, 26, 4, 1, 5, "日期减到 01");
// S8: 日期从 01 减到 00.
tap_once(S8);
assert_setting_date_screen(5, 26, 4, 0, 5, "日期减到 00");
// S8: 日期从 00 回绕到 31.
tap_once(S8);
assert_setting_date_screen(5, 26, 4, 31, 5, "日期减到非法的 31");
// S4: 尝试保存 4 月 31 日.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "4 月 31 日应拒绝写回");

// 场景 3. 月份为 00 时应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "月份为 00 前切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
assert_setting_date_screen(1, 26, 6, 5, 5, "进入月份为 00 设置态");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x6: 月份从 06 减到 00.
tap_many(S8, 6);
assert_setting_date_screen(3, 26, 0, 5, 5, "月份减到 00");
// S4: 尝试保存月份 00.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "月份 00 应拒绝写回");

// 场景 4. 日期为 00 时应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "日期为 00 前切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
assert_setting_date_screen(1, 26, 6, 5, 5, "进入日期为 00 设置态");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S7: 切到日期字段.
tap_once(S7);
assert_setting_date_screen(5, 26, 6, 5, 5, "切到日期字段");
// S8 x5: 日期从 05 减到 00.
tap_many(S8, 5);
assert_setting_date_screen(5, 26, 6, 0, 5, "日期减到 00");
// S4: 尝试保存日期 00.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "日期 00 应拒绝写回");

// 场景 5. 星期为 00 时应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "星期为 00 前切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
assert_setting_date_screen(1, 26, 6, 5, 5, "进入星期为 00 设置态");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S7: 切到日期字段.
tap_once(S7);
assert_setting_date_screen(5, 26, 6, 5, 5, "切到日期字段");
// S7: 切到星期字段.
tap_once(S7);
assert_setting_date_screen(7, 26, 6, 5, 5, "切到星期字段");
// S8 x5: 星期从 05 减到 00.
tap_many(S8, 5);
assert_setting_date_screen(7, 26, 6, 5, 0, "星期减到 00");
// S4: 尝试保存星期 00.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "星期 00 应拒绝写回");

// 场景 6. 如果日期非法, 那么整次保存都应失败.
boot();
set_rtc(7, 8, 9);
let start_ns = sim_time_ns();
assert_time_screen(7, 8, 9, "整次保存失败前的时间");
// S4: 进入时间设置态.
tap_once(S4);
assert_setting_time_screen(3, 7, 8, 9, "进入时间设置态准备同时改时间和日期");
// S9: 小时从 07 加到 08.
tap_once(S9);
assert_setting_time_screen(3, 8, 8, 9, "小时先改到 08");
// S7: 切到分钟字段.
tap_once(S7);
assert_setting_time_screen(5, 8, 8, 9, "切到分钟字段");
// S9: 分钟从 08 加到 09.
tap_once(S9);
assert_setting_time_screen(5, 8, 9, 9, "分钟先改到 09");
// S7: 切到秒字段.
tap_once(S7);
assert_setting_time_screen(7, 8, 9, 9, "切到秒字段");
// S9: 秒从 09 加到 10.
tap_once(S9);
assert_setting_time_screen(7, 8, 9, 10, "秒先改到 10");
// S5: 切到日期设置页, 准备制造非法日期.
tap_once(S5);
assert_setting_date_screen(1, 26, 6, 5, 5, "切到日期设置页");
// S7: 切到月份字段.
tap_once(S7);
assert_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
assert_setting_date_screen(3, 26, 4, 5, 5, "月份减到 04");
// S7: 切到日期字段.
tap_once(S7);
assert_setting_date_screen(5, 26, 4, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
assert_setting_date_screen(5, 26, 4, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
assert_setting_date_screen(5, 26, 4, 1, 5, "日期减到 01");
// S8: 日期从 01 减到 00.
tap_once(S8);
assert_setting_date_screen(5, 26, 4, 0, 5, "日期减到 00");
// S8: 日期从 00 回绕到 31, 形成非法日期.
tap_once(S8);
assert_setting_date_screen(5, 26, 4, 31, 5, "日期变成非法的 31");
// S4: 尝试把已修改的时间和非法日期一起保存.
tap_once(S4);
assert_date_screen(26, 6, 5, 5, "非法日期不应写回日期");
// S5: 切回时间页, 检查之前改过的时间也没有保存.
tap_once(S5);
assert_time_keeps_running(7, 8, 9, start_ns, 8, 9, 10, "非法日期也不应写回时间");
