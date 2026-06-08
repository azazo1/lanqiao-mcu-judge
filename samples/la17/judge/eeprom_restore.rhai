key_mode(KEYBOARD);

fn goto_temp_page() {
    tap_key(S4, 80);
    run_ms(120);
}

fn goto_light_page() {
    tap_key(S4, 80);
    run_ms(120);
    tap_key(S4, 80);
    run_ms(120);
}

fn goto_dist_freq_page() {
    tap_key(S4, 80);
    run_ms(120);
    tap_key(S4, 80);
    run_ms(120);
    tap_key(S4, 80);
    run_ms(120);
}

set_rtc(#{
    year: 26,
    month: 7,
    date: 8,
    weekday: 2,
    hour: 23,
    minute: 11,
    second: 22,
    running: false,
});
set_eeprom([0x16, 0x71, 0x0D, 59, 248, 0xF3]);
reset(POWER_RESET);
run_ms(320);

assert_eq(eeprom_byte(0x00), 0x16, "EEPROM 恢复: 锁字节 0 错误");
assert_eq(eeprom_byte(0x01), 0x71, "EEPROM 恢复: 锁字节 1 错误");
assert_eq(eeprom_byte(0x02), 0x0D, "EEPROM 恢复: 配置字节错误");
assert_eq(eeprom_byte(0x03), 59, "EEPROM 恢复: 音速偏移错误");
assert_eq(eeprom_byte(0x04), 248, "EEPROM 恢复: 手动 DAC 错误");
assert_eq(eeprom_byte(0x05), 0xF3, "EEPROM 恢复: 校验字节错误");

assert_eq(display_text(30), "11-11-22", "EEPROM 恢复: 12h 时间显示错误");

goto_temp_page();
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.875-2", "EEPROM 恢复: 温度精度显示错误");

goto_light_page();
tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "1-248", "EEPROM 恢复: 手动光控显示错误");
assert_eq(da_value(), 248, "EEPROM 恢复: 手动 DAC 数值错误");

goto_dist_freq_page();
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_regex(display_text(30), "^\\d{3}-\\d{4}$", "EEPROM 恢复: 液位风机页格式错误");
assert_in(parse_int(display_text()[0..3]), 38..=40, "EEPROM 恢复: 距离显示超出范围");
assert_in(parse_int(display_text()[4..8]), 1190..=1210, "EEPROM 恢复: 频率显示超出范围");
