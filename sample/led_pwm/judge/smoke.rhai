run_ms(220);
assert(regex_is_match(display_text(30), "^\\d{3}$"), "上电占空比显示格式");
assert(parse_int(display_text(30)[0..3]) == 0, "上电占空比数值");

let initial_stats = watch_led_stats(L1, 40);
assert(initial_stats.pwm_frequency_hz >= 950.0 && initial_stats.pwm_frequency_hz <= 1050.0, "上电 L1 PWM 频率约 1kHz");
assert(initial_stats.duty_percent >= 8.0 && initial_stats.duty_percent <= 12.0, "上电占空比约 10%");

tap_key(S9, 80);
run_ms(120);
assert(parse_int(display_text(30)[0..3]) == 1, "S9 按下后占空比数值增加");

let increased_stats = watch_led_stats(L1, 40);
assert(increased_stats.pwm_frequency_hz >= 950.0 && increased_stats.pwm_frequency_hz <= 1050.0, "S9 按下后 PWM 频率保持约 1kHz");
assert(increased_stats.duty_percent >= 18.0 && increased_stats.duty_percent <= 22.0, "S9 按下后占空比约 20%");

tap_key(S8, 80);
run_ms(120);
assert(parse_int(display_text(30)[0..3]) == 0, "S8 按下后占空比数值恢复");

let restored_stats = watch_led_stats(L1, 40);
assert(restored_stats.pwm_frequency_hz >= 950.0 && restored_stats.pwm_frequency_hz <= 1050.0, "S8 按下后 PWM 频率保持约 1kHz");
assert(restored_stats.duty_percent >= 8.0 && restored_stats.duty_percent <= 12.0, "S8 按下后占空比回到约 10%");
