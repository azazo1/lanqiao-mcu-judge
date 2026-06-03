run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert_in(stats.changes, 9..=11, "1 秒内 L1 线路变化次数约 10");
