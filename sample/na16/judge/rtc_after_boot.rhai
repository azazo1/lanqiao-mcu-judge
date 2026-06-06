fn settle() {
    run_ms(320);
}

fn assert_time_page(hour, minute, second, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{2}-\\d{2}-\\d{2}$", label + ": 时间格式错误");
    assert_eq(parse_int(text[0..2]), hour, label + ": 小时显示错误");
    assert_eq(parse_int(text[3..5]), minute, label + ": 分钟显示错误");
    assert_eq(parse_int(text[6..8]), second, label + ": 秒显示错误");
}

set_rtc(23, 59, 50);
settle();
assert_time_page(23, 59, 50, "上电 RTC 显示");
