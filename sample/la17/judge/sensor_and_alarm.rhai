key_mode(KEYBOARD);

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

fn assert_adc_page(left_min, left_max, right_min, right_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{3}-\\d{3}$", label + " ADC 页面格式");
    assert_in(parse_int(text[0..3]), left_min..=left_max, label + " AIN1");
    assert_in(parse_int(text[4..7]), right_min..=right_max, label + " AIN3");
}

fn assert_dac_page(mode, dac_min, dac_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d-\\d{3}$", label + " DAC 页面格式");
    assert_eq(parse_int(text[0..1]), mode, label + " 模式位");
    assert_in(parse_int(text[2..5]), dac_min..=dac_max, label + " DAC");
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
set_eeprom([0x16, 0x71, 0x00, 20, 96, 0x49]);
reset(POWER_RESET);
run_ms(320);

goto_light_page();
set_voltage(AIN1, 4.0);
set_voltage(AIN3, 1.0);
run_ms(260);
assert_adc_page(203, 205, 50, 52, "亮环境 ADC 采样");

tap_key(S5, 80);
run_ms(120);
assert_dac_page(0, 32, 32, "亮环境自动光控");
assert_eq(da_value(), 32, "亮环境自动光控 DAC");
let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 8..=16, "亮环境下 L6 占空比");

set_voltage(AIN1, 1.0);
set_voltage(AIN3, 4.0);
run_ms(260);
assert_dac_page(0, 224, 224, "暗环境自动光控");
assert_eq(da_value(), 224, "暗环境自动光控 DAC");
let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 84..=92, "暗环境下 L6 占空比");

set_voltage(AIN1, 2.0);
set_voltage(AIN3, 2.0);
run_ms(260);
assert_dac_page(0, 32, 32, "相等阈值环境应归入非低照分支");
assert_eq(da_value(), 32, "相等阈值环境 DAC");

reset(POWER_RESET);
run_ms(320);
goto_dist_freq_page();
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_dist_freq_page(34, 36, 1190, 1210, "默认音速下的液位风机页");

uart_write("$SET|SV:360;");
run_ms(260);
assert_eq(uart_take(), "$OK|SET;", "串口修改音速");
run_ms(260);
assert_dist_freq_page(36, 38, 1190, 1210, "音速提高后的距离显示");

set_distance_cm(15);
run_ms(320);
let dist_edge_low = watch_led_stats(L7, 300);
assert_eq(dist_edge_low.changes, 0, "距离等于 15 时不应告警");
assert(!led_on(L7), "距离等于 15 时 L7 应熄灭");

set_distance_cm(70);
run_ms(320);
let dist_edge_high = watch_led_stats(L7, 300);
assert_eq(dist_edge_high.changes, 0, "距离等于 70 时不应告警");
assert(!led_on(L7), "距离等于 70 时 L7 应熄灭");

set_distance_cm(10);
run_ms(320);
let dist_alarm = watch_led_stats(L7, 1200);
assert_in(dist_alarm.changes, 3..=6, "液位告警闪烁");

set_distance_cm(35);
run_ms(320);
let dist_clear = watch_led_stats(L7, 300);
assert_eq(dist_clear.changes, 0, "距离恢复正常后 L7 不再闪烁");
assert(!led_on(L7), "距离恢复正常后 L7 应熄灭");

set_frequency_hz(2200);
run_ms(1300);
let freq_alarm = watch_led_stats(L8, 700);
assert_in(freq_alarm.changes, 4..=7, "风机告警闪烁");

set_frequency_hz(1200);
run_ms(1300);
let freq_clear = watch_led_stats(L8, 300);
assert_eq(freq_clear.changes, 0, "频率恢复正常后 L8 不再闪烁");
assert(!led_on(L8), "频率恢复正常后 L8 应熄灭");

set_frequency_hz(1000);
run_ms(1300);
let freq_edge_low = watch_led_stats(L8, 300);
assert_eq(freq_edge_low.changes, 0, "频率等于 1000 时不应告警");
assert(!led_on(L8), "频率等于 1000 时 L8 应熄灭");

set_frequency_hz(2000);
run_ms(1300);
let freq_edge_high = watch_led_stats(L8, 300);
assert_eq(freq_edge_high.changes, 0, "频率等于 2000 时不应告警");
assert(!led_on(L8), "频率等于 2000 时 L8 应熄灭");
