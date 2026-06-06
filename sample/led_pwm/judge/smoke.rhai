add_marker("boot");
run_ms(220);
add_marker("boot_display_ready");
assert_regex(display_text(30), "^\\d{3}$", "上电检查: 占空比显示格式错误");
assert_eq(display_number(1, 3, 30), 0, "上电检查: 占空比数值错误");

let initial_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_10pct");
assert_in(initial_stats.pwm_frequency_hz, 950..=1050, "上电检查: L1 PWM 频率超出范围");
assert_in(initial_stats.duty_percent, 8..=12, "上电检查: 占空比超出范围");

tap_key(S9, 80);
assert_eq(display_number(1, 3, 30), 1, "S9 按下: 占空比数值错误");

let increased_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_20pct");
assert_in(increased_stats.pwm_frequency_hz, 950..=1050, "S9 按下: PWM 频率超出范围");
assert_in(increased_stats.duty_percent, 18..=22, "S9 按下: 占空比超出范围");

tap_key(S8, 80);
assert_eq(display_number(1, 3, 30), 0, "S8 按下: 占空比恢复错误");

let restored_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_restored");
assert_in(restored_stats.pwm_frequency_hz, 950..=1050, "S8 按下: PWM 频率超出范围");
assert_in(restored_stats.duty_percent, 8..=12, "S8 按下: 占空比超出范围");
