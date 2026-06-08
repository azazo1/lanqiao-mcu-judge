key_mode(KEYBOARD);

fn assert_page_led(page, label) {
    assert_eq(led_on(L1), page == 0, label + ": L1 页面指示状态错误");
    assert_eq(led_on(L2), page == 1, label + ": L2 页面指示状态错误");
    assert_eq(led_on(L3), page == 2, label + ": L3 页面指示状态错误");
    assert_eq(led_on(L4), page == 3, label + ": L4 页面指示状态错误");
}

fn assert_adc_page(left_min, left_max, right_min, right_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{3}-\\d{3}$", label + ": ADC 页面格式错误");
    assert_in(parse_int(text[0..3]), left_min..=left_max, label + ": AIN1 显示超出范围");
    assert_in(parse_int(text[4..7]), right_min..=right_max, label + ": AIN3 显示超出范围");
}

fn assert_dac_page(mode, dac_min, dac_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d-\\d{3}$", label + ": DAC 页面格式错误");
    assert_eq(parse_int(text[0..1]), mode, label + ": 模式位显示错误");
    assert_in(parse_int(text[2..5]), dac_min..=dac_max, label + ": DAC 显示超出范围");
}

fn assert_dist_freq_page(dist_min, dist_max, freq_min, freq_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{3}-\\d{4}$", label + ": 距离频率页面格式错误");
    assert_in(parse_int(text[0..3]), dist_min..=dist_max, label + ": 距离显示超出范围");
    assert_in(parse_int(text[4..8]), freq_min..=freq_max, label + ": 频率显示超出范围");
}

set_rtc(#{
    year: 26,
    month: 6,
    date: 5,
    weekday: 4,
    hour: 23,
    minute: 59,
    second: 50,
    running: false,
});
set_eeprom([0, 0, 0, 0, 0, 0]);
reset(POWER_RESET);
run_ms(320);

assert_eq(eeprom_byte(0x00), 0x16, "上电检查: 锁字节 0 错误");
assert_eq(eeprom_byte(0x01), 0x71, "上电检查: 锁字节 1 错误");
assert_eq(eeprom_byte(0x02), 0x00, "上电检查: 默认配置字节错误");
assert_eq(eeprom_byte(0x03), 20, "上电检查: 默认音速偏移错误");
assert_eq(eeprom_byte(0x04), 96, "上电检查: 默认手动 DAC 错误");
assert_eq(eeprom_byte(0x05), 0x49, "上电检查: 默认校验字节错误");

assert_eq(display_text(30), "23-59-50", "上电检查: 默认时间页显示错误");
assert_page_led(0, "上电页面指示");

let heartbeat = watch_led_stats(L5, 2100);
assert_in(heartbeat.changes, 3..=5, "上电检查: L5 心跳灯翻转次数超出范围");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "26-06-05", "S5 短按: 日期页显示错误");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "23-59-50", "S5 再次短按: 时间页显示错误");

tap_key(S4, 80);
run_ms(120);
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.500-0", "温度页检查: 默认精度显示错误");
assert_page_led(1, "温度页页面指示");

tap_key(S4, 80);
run_ms(120);
set_voltage(AIN1, 4.0);
set_voltage(AIN3, 1.0);
run_ms(260);
assert_adc_page(203, 205, 50, 52, "光控页 ADC 子页面");
assert_page_led(2, "光控页页面指示");

tap_key(S5, 80);
run_ms(120);
assert_dac_page(0, 32, 32, "自动光控下的 DAC 子页面");
assert_eq(da_value(), 32, "自动光控检查: 亮环境 DAC 错误");

tap_key(S4, 80);
run_ms(120);
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_dist_freq_page(34, 36, 1190, 1210, "液位风机页显示");
assert_page_led(3, "液位风机页页面指示");
