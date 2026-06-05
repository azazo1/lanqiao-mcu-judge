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

// 场景 1. 非闰年 2 月 29 日应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "非闰年场景切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入日期设置态");
// S8: 年份从 26 减到 25, 构造非闰年.
tap_once(S8);
wait_setting_date_screen(1, 25, 6, 5, 5, "年份减到非闰年 25");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 25, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
wait_setting_date_screen(3, 25, 4, 5, 5, "月份减到 04");
// S8 x2: 月份从 04 减到 02.
tap_many(S8, 2);
wait_setting_date_screen(3, 25, 2, 5, 5, "月份减到 02");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 25, 2, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
wait_setting_date_screen(5, 25, 2, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(5, 25, 2, 1, 5, "日期减到 01");
// S8 x2: 日期从 01 回绕到 30.
tap_many(S8, 2);
wait_setting_date_screen(5, 25, 2, 30, 5, "日期回绕到 30");
// S8: 日期从 30 减到 29.
tap_once(S8);
wait_setting_date_screen(5, 25, 2, 29, 5, "日期减到非法的 29");
// S7: 切到星期字段.
tap_once(S7);
wait_setting_date_screen(7, 25, 2, 29, 5, "切到星期字段");
// S9: 星期从 05 加到 06.
tap_once(S9);
wait_setting_date_screen(7, 25, 2, 29, 6, "星期加到 06");
// S4: 尝试保存非法日期.
tap_once(S4);
wait_date_screen(26, 6, 5, 5, "非闰年 2 月 29 日应拒绝写回");

// 场景 2. 4 月 31 日应拒绝写回.
boot();
// S5: 切到日期页.
tap_once(S5);
wait_date_screen(26, 6, 5, 5, "4 月场景切到日期页");
// S4: 进入日期设置态.
tap_once(S4);
wait_setting_date_screen(1, 26, 6, 5, 5, "进入 4 月设置态");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
wait_setting_date_screen(3, 26, 4, 5, 5, "月份减到 04");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 26, 4, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 1, 5, "日期减到 01");
// S8 x2: 日期从 01 回绕到 30.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 30, 5, "日期回绕到 30");
// S8: 日期从 30 减到 31.
tap_once(S8);
wait_setting_date_screen(5, 26, 4, 31, 5, "日期减到非法的 31");
// S4: 尝试保存 4 月 31 日.
tap_once(S4);
wait_date_screen(26, 6, 5, 5, "4 月 31 日应拒绝写回");

// 场景 3. 如果日期非法, 那么整次保存都应失败.
boot();
set_rtc(7, 8, 9);
wait_time_screen(7, 8, 9, "整次保存失败前的时间");
// S4: 进入时间设置态.
tap_once(S4);
wait_setting_time_screen(3, 7, 8, 9, "进入时间设置态准备同时改时间和日期");
// S9: 小时从 07 加到 08.
tap_once(S9);
wait_setting_time_screen(3, 8, 8, 9, "小时先改到 08");
// S7: 切到分钟字段.
tap_once(S7);
wait_setting_time_screen(5, 8, 8, 9, "切到分钟字段");
// S9: 分钟从 08 加到 09.
tap_once(S9);
wait_setting_time_screen(5, 8, 9, 9, "分钟先改到 09");
// S7: 切到秒字段.
tap_once(S7);
wait_setting_time_screen(7, 8, 9, 9, "切到秒字段");
// S9: 秒从 09 加到 10.
tap_once(S9);
wait_setting_time_screen(7, 8, 9, 10, "秒先改到 10");
// S5: 切到日期设置页, 准备制造非法日期.
tap_once(S5);
wait_setting_date_screen(1, 26, 6, 5, 5, "切到日期设置页");
// S7: 切到月份字段.
tap_once(S7);
wait_setting_date_screen(3, 26, 6, 5, 5, "切到月份字段");
// S8 x2: 月份从 06 减到 04.
tap_many(S8, 2);
wait_setting_date_screen(3, 26, 4, 5, 5, "月份减到 04");
// S7: 切到日期字段.
tap_once(S7);
wait_setting_date_screen(5, 26, 4, 5, 5, "切到日期字段");
// S8 x2: 日期从 05 减到 03.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 3, 5, "日期减到 03");
// S8 x2: 日期从 03 减到 01.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 1, 5, "日期减到 01");
// S8 x2: 日期从 01 回绕到 30.
tap_many(S8, 2);
wait_setting_date_screen(5, 26, 4, 30, 5, "日期回绕到 30");
// S8: 日期从 30 减到 31, 形成非法日期.
tap_once(S8);
wait_setting_date_screen(5, 26, 4, 31, 5, "日期变成非法的 31");
// S4: 尝试把已修改的时间和非法日期一起保存.
tap_once(S4);
wait_date_screen(26, 6, 5, 5, "非法日期不应写回日期");
// S5: 切回时间页, 检查之前改过的时间也没有保存.
tap_once(S5);
wait_time_screen(7, 8, 9, "非法日期也不应写回时间");
