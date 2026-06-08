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
assert_eq(display_text(30), "1-096", "手动 DAC 边界: 初始页面显示错误");

tap_many(S9, 20);
assert_eq(display_text(30), "1-000", "手动 DAC 下界: 页面显示错误");
assert_eq(da_value(), 0, "手动 DAC 下界: DAC 数值错误");
tap_key(S9, 80);
run_ms(120);
assert_eq(display_text(30), "1-000", "手动 DAC 下界保持: 页面显示错误");
assert_eq(da_value(), 0, "手动 DAC 下界保持: DAC 数值错误");

let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 0..=5, "手动 DAC 下界: PWM 占空比超出范围");

tap_many(S8, 40);
assert_eq(display_text(30), "1-255", "手动 DAC 上界: 页面显示错误");
assert_eq(da_value(), 255, "手动 DAC 上界: DAC 数值错误");
tap_key(S8, 80);
run_ms(120);
assert_eq(display_text(30), "1-255", "手动 DAC 上界保持: 页面显示错误");
assert_eq(da_value(), 255, "手动 DAC 上界保持: DAC 数值错误");

let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 95..=100, "手动 DAC 上界: PWM 占空比超出范围");

double_click(S12);
assert_eq(display_text(30), "0-032", "切回自动光控: 页面显示错误");
