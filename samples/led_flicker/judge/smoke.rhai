run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert_in(stats.change_frequency_hz, 9..=11, "L1 闪烁统计: 状态切换频率超出范围");
