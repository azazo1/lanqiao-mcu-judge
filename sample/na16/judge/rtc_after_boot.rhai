run_ms(5);
set_rtc(23, 59, 50);
run_ms(200);
assert(regex_is_match(display_text(30), "^\\d{2}-\\d{2}-\\d{2}$"), "RTC 显示格式");
assert(parse_int(display_text()[0..2]) == 23, "RTC 小时");
assert_eq_str(display_text()[2..3], "-", "RTC 第1个分隔符");
assert(parse_int(display_text()[3..5]) == 59, "RTC 分钟");
assert_eq_str(display_text()[5..6], "-", "RTC 第2个分隔符");
assert(parse_int(display_text()[6..8]) == 50, "RTC 秒钟");
