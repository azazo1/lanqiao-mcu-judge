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

fn assert_date_text(text, year, month, date, day, label) {
    assert_regex(text, "^\\d{2}\\.\\d{2}\\.\\d{2}\\.\\d{2}$", label + " 日期格式");
    assert_eq(parse_int(text[0..2]), year, label + " 年");
    assert_eq(text[2..3], ".", label + " 第1个分隔点");
    assert_eq(parse_int(text[3..5]), month, label + " 月");
    assert_eq(text[5..6], ".", label + " 第2个分隔点");
    assert_eq(parse_int(text[6..8]), date, label + " 日");
    assert_eq(text[8..9], ".", label + " 第3个分隔点");
    assert_eq(parse_int(text[9..11]), day, label + " 周");
}

fn assert_time_text(text, hour, minute, second, label) {
    assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", label + " 时间格式");
    assert_eq(parse_int(text[2..4]), hour, label + " 小时");
    assert_eq(parse_int(text[5..7]), minute, label + " 分钟");
    assert_eq(parse_int(text[8..10]), second, label + " 秒");
}

fn wait_time_screen(hour, minute, second, label) {
    let expected = time_text(hour, minute, second);
    let dt = wait_until("seg.text", expected, 300_000_000);
    assert_in(dt, 0..=300_000_000, label + " 等待正常态时间显示");
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_time_text(text, hour, minute, second, label);
}

fn wait_setting_time_screen(visible_digit, hour, minute, second, label) {
    let target = "seg.d" + visible_digit + ".visible";
    let dt = wait_until(target, true, 1_200_000_000);
    assert_in(dt, 0..=1_200_000_000, label + " 等待时间设置态可见相位");
    let text = display_text();
    assert_eq(text, time_text(hour, minute, second), label + " 可见文本");
    assert_time_text(text, hour, minute, second, label);
}

fn wait_date_screen(year, month, date, day, label) {
    let expected = date_text(year, month, date, day);
    let dt = wait_until("seg.text", expected, 300_000_000);
    assert_in(dt, 0..=300_000_000, label + " 等待正常态日期显示");
    let text = display_text();
    assert_eq(text, expected, label + " 可见文本");
    assert_date_text(text, year, month, date, day, label);
}

fn wait_setting_date_screen(visible_digit, year, month, date, day, label) {
    let target = "seg.d" + visible_digit + ".visible";
    let dt = wait_until(target, true, 1_200_000_000);
    assert_in(dt, 0..=1_200_000_000, label + " 等待日期设置态可见相位");
    let text = display_text();
    assert_eq(text, date_text(year, month, date, day), label + " 可见文本");
    assert_date_text(text, year, month, date, day, label);
}

// 场景 1. 从时间设置页切到日期设置页时, 选中字段应重置到年份.
boot();
// S4: 先进入时间设置态.
tap_once(S4);
wait_setting_time_screen(3, 13, 59, 58, "进入时间设置态");
// S7: 把当前选中字段移动到后一个时间字段.
tap_once(S7);
wait_setting_time_screen(5, 13, 59, 58, "切到后一个时间字段");
// S5: 切到日期设置页.
tap_once(S5);
wait_setting_date_screen(1, 26, 6, 5, 5, "切到日期设置页");
// S9: 年份加 1, 用来验证选中字段已经重置到年份.
tap_once(S9);
wait_setting_date_screen(1, 27, 6, 5, 5, "切页后年份字段生效");
// S4: 退出设置态并保存.
tap_once(S4);
wait_date_screen(27, 6, 5, 5, "保存年份修改");

// 场景 2. 合法闰年日期应允许保存.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "闰年场景切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入日期设置态");
// S8: 年份从 26 减到 25.
tap_once(S8);
wait_setting_date_screen(1, 25, 6, 5, 5, "年份减到 25");
// S8: 年份从 25 减到 24.
tap_once(S8);
wait_setting_date_screen(1, 24, 6, 5, 5, "年份减到 24");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 24, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 6 减到 4.
tap_many(S8, 2);
wait_setting_date_screen(3, 24, 4, 5, 5, "月份减到 04");
// S8 x2: 月份从 4 减到 2.
tap_many(S8, 2);
wait_setting_date_screen(3, 24, 2, 5, 5, "月份减到 02");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 24, 2, 5, 5, "切到日期字段");
// S8 x2: 日期从 5 减到 3.
tap_many(S8, 2);
wait_setting_date_screen(5, 24, 2, 3, 5, "日期减到 03");
// S8 x2: 日期从 3 减到 1.
tap_many(S8, 2);
wait_setting_date_screen(5, 24, 2, 1, 5, "日期减到 01");
// S8 x2: 日期从 1 回绕到 30.
tap_many(S8, 2);
wait_setting_date_screen(5, 24, 2, 30, 5, "日期回绕到 30");
// S8: 日期从 30 减到 29.
tap_once(S8);
wait_setting_date_screen(5, 24, 2, 29, 5, "日期减到 29");
// S7: 切到星期字段.
tap_once(S7);
wait_setting_date_screen(7, 24, 2, 29, 5, "切到星期字段");
// S9: 星期从 5 加到 6.
tap_once(S9);
wait_setting_date_screen(7, 24, 2, 29, 6, "星期加到 06");
// S4: 退出设置态并保存.
tap_once(S4);
wait_date_screen(24, 2, 29, 6, "保存合法闰年日期");

// 场景 3. 年份字段应支持下边界和上边界回绕.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "年份边界切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入年份边界设置态");
// S8 x6: 年份从 26 减到 20.
tap_many(S8, 6);
wait_setting_date_screen(1, 20, 6, 5, 5, "年份减到 20");
// S8 x10: 年份从 20 减到 10.
tap_many(S8, 10);
wait_setting_date_screen(1, 10, 6, 5, 5, "年份减到 10");
// S8 x9: 年份从 10 减到 01.
tap_many(S8, 9);
wait_setting_date_screen(1, 1, 6, 5, 5, "年份减到 01");
// S8: 年份从 01 回绕到 99.
tap_once(S8);
wait_setting_date_screen(1, 99, 6, 5, 5, "年份从 01 回绕到 99");
// S9: 年份从 99 回绕到 01.
tap_once(S9);
wait_setting_date_screen(1, 1, 6, 5, 5, "年份从 99 回绕到 01");
// S4: 保存年份边界结果.
tap_once(S4);
wait_date_screen(1, 6, 5, 5, "保存年份边界结果");

// 场景 4. 月份字段应支持下边界和上边界回绕.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "月份边界切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入月份边界设置态");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x3: 月份从 06 减到 03.
tap_many(S8, 3);
wait_setting_date_screen(3, 26, 3, 5, 5, "月份减到 03");
// S8 x2: 月份从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(3, 26, 1, 5, 5, "月份减到 01");
// S8: 月份从 01 回绕到 12.
tap_once(S8);
wait_setting_date_screen(3, 26, 12, 5, 5, "月份从 01 回绕到 12");
// S9: 月份从 12 回绕到 01.
tap_once(S9);
wait_setting_date_screen(3, 26, 1, 5, 5, "月份从 12 回绕到 01");
// S4: 保存月份边界结果.
tap_once(S4);
wait_date_screen(26, 1, 5, 5, "保存月份边界结果");

// 场景 5. 日期字段应支持下边界和上边界回绕.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "日期边界切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入日期边界设置态");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 26, 6, 5, 5, "日期边界切到月份字段");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 26, 6, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 6, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 6, 1, 5, "日期减到 01");
// S8: 日期从 01 回绕到 31.
tap_once(S8);
wait_setting_date_screen(5, 26, 6, 31, 5, "日期从 01 回绕到 31");
// S9: 日期从 31 回绕到 01.
tap_once(S9);
wait_setting_date_screen(5, 26, 6, 1, 5, "日期从 31 回绕到 01");
// S4: 保存日期边界结果.
tap_once(S4);
wait_date_screen(26, 6, 1, 5, "保存日期边界结果");

// 场景 6. 星期字段应支持下边界和上边界回绕.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "星期边界切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入星期边界设置态");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 26, 6, 5, 5, "星期边界切到月份字段");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 26, 6, 5, 5, "星期边界切到日期字段");
// S7: 切到星期字段.
tap_once(S7);
wait_setting_date_screen(7, 26, 6, 5, 5, "切到星期字段");
// S8 x2: 星期从 05 减到 03.
tap_many(S8, 2);
wait_setting_date_screen(7, 26, 6, 5, 3, "星期减到 03");
// S8 x2: 星期从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(7, 26, 6, 5, 1, "星期减到 01");
// S8: 星期从 01 回绕到 07.
tap_once(S8);
wait_setting_date_screen(7, 26, 6, 5, 7, "星期从 01 回绕到 07");
// S9: 星期从 07 回绕到 01.
tap_once(S9);
wait_setting_date_screen(7, 26, 6, 5, 1, "星期从 07 回绕到 01");
// S4: 保存星期边界结果.
tap_once(S4);
wait_date_screen(26, 6, 5, 1, "保存星期边界结果");
