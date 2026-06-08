run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert_in(stats.changes, 9..=11, "L1 闪烁统计: 1秒内线路变化次数超出范围");
