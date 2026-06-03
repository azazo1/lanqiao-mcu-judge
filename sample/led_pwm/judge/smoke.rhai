run_ms(220);
assert(regex_is_match(display_text(30), "^\\d{3}$"), "上电占空比显示格式");
assert_eq(parse_int(display_text(30)[0..3]), 0, "上电占空比数值");

let initial_stats = watch_led_stats(L1, 40);
assert_in(initial_stats.pwm_frequency_hz, 950..=1050, "上电 L1 PWM 频率约 1kHz");
assert_in(initial_stats.duty_percent, 8..=12, "上电占空比约 10%");

tap_key(S9, 80);
run_ms(120);
assert_eq(parse_int(display_text(30)[0..3]), 1, "S9 按下后占空比数值增加");

let increased_stats = watch_led_stats(L1, 40);
assert_in(increased_stats.pwm_frequency_hz, 950..=1050, "S9 按下后 PWM 频率保持约 1kHz");
assert_in(increased_stats.duty_percent, 18..=22, "S9 按下后占空比约 20%");

tap_key(S8, 80);
run_ms(120);
assert_eq(parse_int(display_text(30)[0..3]), 0, "S8 按下后占空比数值恢复");

let restored_stats = watch_led_stats(L1, 40);
assert_in(restored_stats.pwm_frequency_hz, 950..=1050, "S8 按下后 PWM 频率保持约 1kHz");
assert_in(restored_stats.duty_percent, 8..=12, "S8 按下后占空比回到约 10%");
