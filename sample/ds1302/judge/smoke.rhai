key_mode(KEYBOARD);

fn settle() {
    run_ms(240);
}

fn tap_once(key) {
    tap_key(key, 80);
}

fn assert_time_text(text, hour, minute, second, label) {
    assert(regex_is_match(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$"), label + " 时间格式");
    assert_eq(parse_int(text[2..4]), hour, label + " 小时");
    assert_eq(text[4..5], ".", label + " 第1个分隔点");
    assert_eq(parse_int(text[5..7]), minute, label + " 分钟");
    assert_eq(text[7..8], ".", label + " 第2个分隔点");
    assert_eq(parse_int(text[8..10]), second, label + " 秒");
}

fn assert_time_screen(hour, minute, second, label) {
    assert_time_text(display_text(30), hour, minute, second, label);
}

fn assert_date_text(text, year, month, date, day, label) {
    assert(regex_is_match(text, "^\\d{2}\\.\\d{2}\\.\\d{2}\\.\\d{2}$"), label + " 日期格式");
    assert_eq(parse_int(text[0..2]), year, label + " 年");
    assert_eq(text[2..3], ".", label + " 第1个分隔点");
    assert_eq(parse_int(text[3..5]), month, label + " 月");
    assert_eq(text[5..6], ".", label + " 第2个分隔点");
    assert_eq(parse_int(text[6..8]), date, label + " 日");
    assert_eq(text[8..9], ".", label + " 第3个分隔点");
    assert_eq(parse_int(text[9..11]), day, label + " 周");
}

fn assert_date_screen(year, month, date, day, label) {
    assert_date_text(display_text(30), year, month, date, day, label);
}

// 上电后先检查默认时间页.
settle();
assert_time_screen(13, 59, 58, "上电默认时间页");

// 再等 2.2s, 检查跨整点后的时间.
run_ms(2200);
assert_time_screen(14, 0, 0, "时间递进和整点进位");

// 切到日期页, 检查初始化日期.
// S5: 从时间页切到日期页.
tap_once(S5);
assert_date_screen(26, 6, 5, 5, "日期页显示");

// 再切回时间页, 页面应该保持可读.
// S5: 从日期页切回时间页.
tap_once(S5);
assert_time_screen(14, 0, 0, "切回时间页");
