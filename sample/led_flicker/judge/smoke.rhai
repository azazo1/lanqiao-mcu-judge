run_ms(20);
assert_eq_int(watch_led_changes(L1, 1000), 29, "1 秒内 L1 线路变化次数");
