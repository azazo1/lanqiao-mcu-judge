key_mode(KEYBOARD);

fn double_click(key) {
    tap_key(key, 80);
    run_ms(120);
    tap_key(key, 80);
    run_ms(220);
}

fn goto_light_page() {
    tap_key(S4, 80);
    run_ms(120);
    tap_key(S4, 80);
    run_ms(120);
}

fn tap_many(key, count) {
    let i = 0;
    while i < count {
        tap_key(key, 80);
        run_ms(120);
        i += 1;
    }
}

set_rtc(#{
    year: 26,
    month: 6,
    date: 5,
    weekday: 4,
    hour: 12,
    minute: 0,
    second: 0,
    running: false,
});
set_eeprom([0x16, 0x71, 0x08, 20, 96, 0x41]);
reset(POWER_RESET);
run_ms(320);

goto_light_page();
tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "1-096", "默认进入手动光控 DAC 子页面");

tap_many(S9, 20);
assert_eq(display_text(30), "1-000", "手动 DAC 下界饱和到 000");
assert_eq(da_value(), 0, "手动 DAC 下界值");
tap_key(S9, 80);
run_ms(120);
assert_eq(display_text(30), "1-000", "继续减小后仍保持 000");
assert_eq(da_value(), 0, "继续减小后 DAC 仍保持 0");

let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 0..=5, "DAC 为 0 时的 PWM 占空比");

tap_many(S8, 40);
assert_eq(display_text(30), "1-255", "手动 DAC 上界饱和到 255");
assert_eq(da_value(), 255, "手动 DAC 上界值");
tap_key(S8, 80);
run_ms(120);
assert_eq(display_text(30), "1-255", "继续增大后仍保持 255");
assert_eq(da_value(), 255, "继续增大后 DAC 仍保持 255");

let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 95..=100, "DAC 为 255 时的 PWM 占空比");

double_click(S12);
assert_eq(display_text(30), "0-032", "切回自动光控后显示自动模式结果");
