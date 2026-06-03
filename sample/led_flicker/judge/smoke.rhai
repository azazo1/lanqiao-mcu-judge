run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert(stats.changes >= 9 && stats.changes <= 11, "1 秒内 L1 线路变化次数约 10");
