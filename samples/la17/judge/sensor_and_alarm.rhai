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
assert_eq(da_value(), 32, "亮环境自动光控: DAC 错误");
let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 8..=16, "亮环境自动光控: L6 占空比超出范围");

set_voltage(AIN1, 1.0);
set_voltage(AIN3, 4.0);
run_ms(260);
assert_dac_page(0, 224, 224, "暗环境自动光控");
assert_eq(da_value(), 224, "暗环境自动光控: DAC 错误");
let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 84..=92, "暗环境自动光控: L6 占空比超出范围");

set_voltage(AIN1, 2.0);
set_voltage(AIN3, 2.0);
run_ms(260);
assert_dac_page(0, 32, 32, "相等阈值环境应归入非低照分支");
assert_eq(da_value(), 32, "相等阈值环境: DAC 错误");

reset(POWER_RESET);
run_ms(320);
goto_dist_freq_page();
jumper_on(NET_SIG, SIG_OUT);
set_distance_cm(35);
set_frequency_hz(1200);
run_ms(1300);
assert_dist_freq_page(34, 36, 1190, 1210, "默认音速下的液位风机页");

uart_write("$SET|SV:320;");
run_ms(260);
assert_eq(uart_take(), "$OK|SET;", "串口设置: 音速下界回复错误");
run_ms(260);
assert_dist_freq_page(32, 34, 1190, 1210, "下界音速下的距离显示");

uart_write("$SET|SV:360;");
run_ms(260);
assert_eq(uart_take(), "$OK|SET;", "串口设置: 音速回复错误");
run_ms(260);
assert_dist_freq_page(36, 38, 1190, 1210, "音速提高后的距离显示");

uart_write("$SET|SV:379;");
run_ms(260);
assert_eq(uart_take(), "$OK|SET;", "串口设置: 音速上界回复错误");
run_ms(260);
assert_dist_freq_page(38, 40, 1190, 1210, "上界音速下的距离显示");

set_distance_cm(16);
run_ms(320);
let dist_safe_low = watch_led_stats(L7, 300);
assert_eq(dist_safe_low.changes, 0, "距离 16cm: L7 告警闪烁错误");
assert(!led_on(L7), "距离 16cm: L7 状态错误, 期望熄灭, 实际点亮");

set_distance_cm(15);
run_ms(320);
let dist_edge_low = watch_led_stats(L7, 300);
assert_eq(dist_edge_low.changes, 0, "距离 15cm: L7 告警闪烁错误");
assert(!led_on(L7), "距离 15cm: L7 状态错误, 期望熄灭, 实际点亮");

set_distance_cm(14);
run_ms(320);
let dist_alarm_low = watch_led_stats(L7, 1200);
assert_in(dist_alarm_low.changes, 3..=6, "距离 14cm: L7 告警闪烁次数超出范围");

set_distance_cm(69);
run_ms(320);
let dist_safe_high = watch_led_stats(L7, 300);
assert_eq(dist_safe_high.changes, 0, "距离 69cm: L7 告警闪烁错误");
assert(!led_on(L7), "距离 69cm: L7 状态错误, 期望熄灭, 实际点亮");

set_distance_cm(70);
run_ms(320);
let dist_edge_high = watch_led_stats(L7, 300);
assert_eq(dist_edge_high.changes, 0, "距离 70cm: L7 告警闪烁错误");
assert(!led_on(L7), "距离 70cm: L7 状态错误, 期望熄灭, 实际点亮");

set_distance_cm(71);
run_ms(320);
let dist_alarm_high = watch_led_stats(L7, 1200);
assert_in(dist_alarm_high.changes, 3..=6, "距离 71cm: L7 告警闪烁次数超出范围");

set_distance_cm(10);
run_ms(320);
let dist_alarm = watch_led_stats(L7, 1200);
assert_in(dist_alarm.changes, 3..=6, "液位告警: L7 闪烁次数超出范围");

set_distance_cm(35);
run_ms(320);
let dist_clear = watch_led_stats(L7, 300);
assert_eq(dist_clear.changes, 0, "距离恢复正常: L7 仍在闪烁");
assert(!led_on(L7), "距离恢复正常: L7 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(2200);
run_ms(1300);
let freq_alarm = watch_led_stats(L8, 700);
assert_in(freq_alarm.changes, 4..=7, "风机告警: L8 闪烁次数超出范围");

set_frequency_hz(1200);
run_ms(1300);
let freq_clear = watch_led_stats(L8, 300);
assert_eq(freq_clear.changes, 0, "频率恢复正常: L8 仍在闪烁");
assert(!led_on(L8), "频率恢复正常: L8 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(999);
run_ms(1300);
let freq_alarm_low = watch_led_stats(L8, 700);
assert_in(freq_alarm_low.changes, 4..=7, "频率 999Hz: L8 告警闪烁次数超出范围");

set_frequency_hz(1000);
run_ms(1300);
let freq_edge_low = watch_led_stats(L8, 300);
assert_eq(freq_edge_low.changes, 0, "频率 1000Hz: L8 告警闪烁错误");
assert(!led_on(L8), "频率 1000Hz: L8 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(1001);
run_ms(1300);
let freq_safe_low = watch_led_stats(L8, 300);
assert_eq(freq_safe_low.changes, 0, "频率 1001Hz: L8 告警闪烁错误");
assert(!led_on(L8), "频率 1001Hz: L8 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(1999);
run_ms(1300);
let freq_safe_high = watch_led_stats(L8, 300);
assert_eq(freq_safe_high.changes, 0, "频率 1999Hz: L8 告警闪烁错误");
assert(!led_on(L8), "频率 1999Hz: L8 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(2000);
run_ms(1300);
let freq_edge_high = watch_led_stats(L8, 300);
assert_eq(freq_edge_high.changes, 0, "频率 2000Hz: L8 告警闪烁错误");
assert(!led_on(L8), "频率 2000Hz: L8 状态错误, 期望熄灭, 实际点亮");

set_frequency_hz(2001);
run_ms(1300);
let freq_alarm_high = watch_led_stats(L8, 700);
assert_in(freq_alarm_high.changes, 4..=7, "频率 2001Hz: L8 告警闪烁次数超出范围");
