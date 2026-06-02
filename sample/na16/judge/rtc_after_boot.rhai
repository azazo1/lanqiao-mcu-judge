run_ms(5);
set_rtc(23, 59, 50);
run_ms(60);
assert_eq_str(display_text(), "23-59-50", "启动后设置 RTC");
