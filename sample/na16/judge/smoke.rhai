key_mode(KEYBOARD);

fn assert_page_led(page, label) {
    assert_eq(led_on(L1), page == 0, label + " L1");
    assert_eq(led_on(L2), page == 1, label + " L2");
    assert_eq(led_on(L3), page == 2, label + " L3");
    assert_eq(led_on(L4), page == 3, label + " L4");
}

fn assert_adc_page(left_min, left_max, right_min, right_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{3}-\\d{3}$", label + " AD 页面格式");
    assert_in(parse_int(text[0..3]), left_min..=left_max, label + " AIN1");
    assert_in(parse_int(text[4..7]), right_min..=right_max, label + " AIN3");
}

fn assert_dist_freq_page(dist_min, dist_max, freq_min, freq_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{3}-\\d{4}$", label + " 距离频率页面格式");
    assert_in(parse_int(text[0..3]), dist_min..=dist_max, label + " 距离");
    assert_in(parse_int(text[4..8]), freq_min..=freq_max, label + " 频率");
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

assert_eq(eeprom_byte(0x00), 0xA5, "上电后锁字节 0");
assert_eq(eeprom_byte(0x01), 0x5A, "上电后锁字节 1");
assert_eq(eeprom_byte(0x02), 0x00, "上电后默认配置字节");
assert_eq(eeprom_byte(0x03), 40, "上电后默认音速配置");
assert_eq(eeprom_byte(0x04), 128, "上电后默认 DAC");
assert_eq(eeprom_byte(0x05), 0xA7, "上电后默认校验和");

assert_eq(display_text(30), "23-59-50", "上电默认时间页");
assert_page_led(0, "上电页面指示");

let heartbeat = watch_led_stats(L5, 2100);
assert_in(heartbeat.changes, 3..=5, "L5 心跳灯翻转次数");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "26-06-05", "S5 短按后切到日期页");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "23-59-50", "再次短按后回到时间页");

tap_key(S4, 80);
run_ms(120);
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.500-0", "温度页默认精度显示");
assert_page_led(1, "温度页页面指示");

tap_key(S4, 80);
run_ms(120);
set_voltage(AIN1, 4.0);
set_voltage(AIN3, 1.0);
run_ms(260);
assert_adc_page(203, 205, 50, 52, "AD 页面显示");
assert_page_led(2, "AD 页页面指示");

tap_key(S4, 80);
run_ms(120);
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_dist_freq_page(34, 36, 1190, 1210, "距离频率页显示");
assert_page_led(3, "距离频率页页面指示");
