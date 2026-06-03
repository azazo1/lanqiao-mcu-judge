set_rtc(23, 59, 50);
run_ms(200);
assert(regex_is_match(display_text(30), "^\\d{2}-\\d{2}-\\d{2}$"), "上电时间格式");
assert_eq(parse_int(display_text()[0..2]), 23, "上电小时");
assert_eq(display_text()[2..3], "-", "上电第1个分隔符");
assert_eq(parse_int(display_text()[3..5]), 59, "上电分钟");
assert_eq(display_text()[5..6], "-", "上电第2个分隔符");
assert_eq(parse_int(display_text()[6..8]), 50, "上电秒钟");

jumper_on(NET_SIG, SIG_OUT);
set_frequency_hz(2200);
run_ms(2000);
assert(led_on(6), "频率异常时 L6 应点亮");

set_temperature_c(25);
set_distance_cm(35);
set_voltage(RB2, 2.3);
run_ms(100);

uart_write("(F,?)");
run_ms(200);
