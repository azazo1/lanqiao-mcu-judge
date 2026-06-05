key_mode(KEYBOARD);

fn goto_temp_page() {
    tap_key(S4, 80);
    run_ms(120);
}

fn assert_temp_precision(temp, precision, label) {
    assert_eq(display_number(1, 6), temp, label + " 温度值");
    assert_eq(display_number(8, 8), precision, label + " 精度等级");
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
set_eeprom([0x16, 0x71, 0x00, 20, 96, 0x49]);
reset(POWER_RESET);
run_ms(320);

goto_temp_page();
set_temperature_c(25.9375);
run_ms(260);
assert_temp_precision(25.500, 0, "精度 0 量化");

tap_key(S8, 80);
run_ms(200);
assert_temp_precision(25.750, 1, "精度 1 量化");

tap_key(S8, 80);
run_ms(200);
assert_temp_precision(25.875, 2, "精度 2 量化");

tap_key(S8, 80);
run_ms(200);
assert_temp_precision(25.937, 3, "精度 3 量化");

set_temperature_c(-5.125);
run_ms(260);
assert_temp_precision(-5.125, 3, "负温显示");

tap_key(S9, 80);
run_ms(200);
assert_temp_precision(-5.000, 2, "负温降到精度 2");

tap_key(S9, 80);
run_ms(200);
assert_temp_precision(-5.000, 1, "负温降到精度 1");

tap_key(S9, 80);
run_ms(200);
assert_temp_precision(-5.000, 0, "负温降到精度 0");
