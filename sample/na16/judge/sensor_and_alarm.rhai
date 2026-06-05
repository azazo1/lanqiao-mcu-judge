key_mode(KEYBOARD);

fn goto_ad_page() {
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
    hour: 12,
    minute: 0,
    second: 0,
    running: false,
});
set_eeprom([0xA5, 0x5A, 0x00, 40, 128, 0xA7]);
reset(POWER_RESET);
run_ms(320);

goto_ad_page();
set_voltage(AIN1, 4.0);
set_voltage(AIN3, 1.0);
run_ms(260);
assert_adc_page(203, 205, 50, 52, "默认 AD 采样");

reset(POWER_RESET);
run_ms(320);
goto_dist_freq_page();
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_dist_freq_page(34, 36, 1190, 1210, "默认音速下的距离频率");

uart_write("@CFG,SPD=360#");
run_ms(260);
assert_eq(uart_take(), "@OK,CFG#", "串口修改音速");
run_ms(260);
assert_dist_freq_page(36, 38, 1190, 1210, "音速提高后的距离显示");

set_distance_cm(10);
run_ms(320);
let dist_alarm = watch_led_stats(L7, 1200);
assert_in(dist_alarm.changes, 3..=6, "距离告警闪烁");

set_distance_cm(35);
run_ms(320);
let dist_clear = watch_led_stats(L7, 300);
assert_eq(dist_clear.changes, 0, "距离恢复正常后 L7 不再闪烁");
assert(!led_on(L7), "距离恢复正常后 L7 应熄灭");

set_frequency_hz(2000);
run_ms(1300);
let freq_alarm = watch_led_stats(L8, 700);
assert_in(freq_alarm.changes, 4..=8, "频率告警闪烁");

set_frequency_hz(1200);
run_ms(1300);
let freq_clear = watch_led_stats(L8, 300);
assert_eq(freq_clear.changes, 0, "频率恢复正常后 L8 不再闪烁");
assert(!led_on(L8), "频率恢复正常后 L8 应熄灭");
