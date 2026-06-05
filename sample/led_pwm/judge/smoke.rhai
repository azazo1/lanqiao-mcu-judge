add_marker("boot");
run_ms(220);
add_marker("boot_display_ready");
assert_regex(display_text(30), "^\\d{3}$", "上电占空比显示格式");
assert_eq(display_number(1, 3, 30), 0, "上电占空比数值");

let initial_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_10pct");
assert_in(initial_stats.pwm_frequency_hz, 950..=1050, "上电 L1 PWM 频率约 1kHz");
assert_in(initial_stats.duty_percent, 8..=12, "上电占空比约 10%");

tap_key(S9, 80);
assert_eq(display_number(1, 3, 30), 1, "S9 按下后占空比数值增加");

let increased_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_20pct");
assert_in(increased_stats.pwm_frequency_hz, 950..=1050, "S9 按下后 PWM 频率保持约 1kHz");
assert_in(increased_stats.duty_percent, 18..=22, "S9 按下后占空比约 20%");

tap_key(S8, 80);
assert_eq(display_number(1, 3, 30), 0, "S8 按下后占空比数值恢复");

let restored_stats = watch_led_stats(L1, 40);
add_marker("measured_pwm_restored");
assert_in(restored_stats.pwm_frequency_hz, 950..=1050, "S8 按下后 PWM 频率保持约 1kHz");
assert_in(restored_stats.duty_percent, 8..=12, "S8 按下后占空比回到约 10%");
