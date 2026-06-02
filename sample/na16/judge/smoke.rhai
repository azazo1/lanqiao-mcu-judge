set_rtc(23, 59, 50);
run_ms(50);
assert_eq_str(display_text(), "23-59-50", "上电时间显示");

jumper_on(NET_SIG, SIG_OUT);
set_frequency_hz(2200);
run_ms(1500);
assert(led_on(6), "频率异常时 L6 应点亮");

set_temperature_c(25);
set_distance_cm(35);
set_voltage(RB2, 2.3);
run_ms(100);

uart_write("(F,?)");
run_ms(200);
