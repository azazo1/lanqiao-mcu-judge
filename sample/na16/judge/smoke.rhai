key_mode(KEYBOARD);

fn settle() {
    run_ms(320);
}

fn sensor_settle() {
    run_ms(1600);
}

fn page_settle() {
    run_ms(160);
}

fn assert_only_data_led(index, label) {
    assert_eq(led_on(L1), index == 1, label + " L1");
    assert_eq(led_on(L2), index == 2, label + " L2");
    assert_eq(led_on(L3), index == 3, label + " L3");
    assert_eq(led_on(L4), index == 4, label + " L4");
}

fn goto_param_menu() {
    tap_key(S4, 80);
    settle();
}

fn goto_std_menu() {
    goto_param_menu();
    tap_key(S4, 80);
    settle();
}

fn assert_time_page(hour, minute, second, label) {
    let text = display_text(30);
    assert_regex(text, "^\\d{2}-\\d{2}-\\d{2}$", label + " 时间格式");
    assert_eq(parse_int(text[0..2]), hour, label + " 小时");
    assert_eq(parse_int(text[3..5]), minute, label + " 分钟");
    assert_eq(parse_int(text[6..8]), second, label + " 秒");
}

fn assert_liq_page(temp, value_min, value_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\s{0,2}-?\\d{1,2}-\\s?\\d{1,2}\\.\\d{1,2}$", label + " 数据格式");
    assert_eq(display_number(1, 4), temp, label + " 左侧数值");
    let value = 0.0 - display_number(5, 8);
    assert(value >= value_min && value <= value_max, label + " 右侧数值");
}

fn assert_weight_page(temp, value_min, value_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\s{0,2}-?\\d{1,2}-\\s?\\d{1,2}\\.\\d{1,2}$", label + " 数据格式");
    assert_eq(display_number(1, 4), temp, label + " 左侧数值");
    let value = display_number(5, 8);
    assert(value >= value_min && value <= value_max, label + " 右侧数值");
}

fn assert_volume_page(spare_min, spare_max, volume_min, volume_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\s?\\d{1,3}-\\s{0,2}\\d{1,2}\\.\\d$", label + " 体积格式");
    assert_in(display_number(1, 3), spare_min..=spare_max, label + " 剩余空间");
    let volume = display_number(5, 8);
    assert(volume >= volume_min && volume <= volume_max, label + " 液体体积");
}

fn assert_display_exact(expected, label) {
    assert_eq(display_text(30), expected, label);
}

fn assert_stable_calibration_after_return(expected, label) {
    assert_display_exact(expected, label);
    tap_key(S4, 80);
    settle();
    tap_key(S4, 80);
    settle();
    tap_key(S4, 80);
    settle();
    assert_display_exact(expected, label + " 再次进入标定界面");
}

set_rtc(23, 59, 50);
settle();

assert_time_page(23, 59, 50, "上电默认时间页");
assert_only_data_led(1, "上电默认指示灯");
assert(!relay_on(), "上电无异常时继电器断开");

set_temperature_c(25);
set_distance_cm(35);
set_voltage(RB2, 2.3);
sensor_settle();

tap_key(S5, 80);
page_settle();
assert_liq_page(25, 0.63, 0.66, "液位页");
assert_only_data_led(2, "液位页指示灯");

tap_key(S5, 80);
page_settle();
assert_volume_page(34, 37, 0.5, 0.7, "体积页");
assert_only_data_led(3, "体积页指示灯");

tap_key(S5, 80);
page_settle();
assert_weight_page(25, 10.6, 10.8, "重量页");
assert_only_data_led(4, "重量页指示灯");

tap_key(S4, 80);
settle();
assert_display_exact("P1-   1.0", "参数页默认显示 H1");

tap_key(S5, 80);
settle();
assert_display_exact("P2-   0.1", "参数页 H2");

tap_key(S5, 80);
settle();
assert_display_exact("P3- 2000", "参数页 F");

tap_key(S5, 80);
settle();
assert_display_exact("P4-    2", "参数页 S");

tap_key(S5, 80);
settle();
assert_display_exact("P5-   1.0", "参数页 r");

tap_key(S5, 80);
settle();
assert_display_exact("P6-   1.0", "参数页 L");

tap_key(S5, 80);
settle();
assert_display_exact("P7-   1.0", "参数页 W");

tap_key(S5, 80);
settle();
assert_display_exact("P8-   1.0", "参数页 H");

tap_key(S4, 80);
settle();
assert_display_exact("E00   0.0", "标定页默认显示");

tap_key(S8, 80);
settle();
tap_key(S8, 80);
settle();
assert_display_exact("E00   0.2", "0 吨标定加 0.2V");

tap_key(S9, 80);
settle();
assert_display_exact("E00   0.1", "0 吨标定减 0.1V");

tap_key(S5, 80);
settle();
assert_display_exact("E05   1.0", "5 吨标定点");

tap_key(S5, 80);
settle();
assert_display_exact("E10   2.0", "10 吨标定点默认值");

tap_key(S9, 80);
settle();
tap_key(S9, 80);
settle();
assert_display_exact("E10   1.8", "10 吨标定减 0.2V");

tap_key(S8, 80);
settle();
assert_display_exact("E10   1.9", "10 吨标定加 0.1V");

tap_key(S5, 80);
settle();
assert_display_exact("E15   4.0", "15 吨标定点");

tap_key(S5, 80);
settle();
assert_display_exact("E20   5.0", "20 吨标定点默认值");

tap_key(S8, 80);
settle();
assert_display_exact("E20   5.1", "20 吨标定加 0.1V");

tap_key(S5, 80);
settle();
tap_key(S9, 80);
settle();
assert_display_exact("E00   0.0", "0 吨标定回到 0.0V");

tap_key(S4, 80);
settle();
assert_only_data_led(1, "退出标定后指示灯");

goto_std_menu();
assert_stable_calibration_after_return("E00   0.0", "合法标定值应保存");
tap_key(S9, 80);
settle();
assert_display_exact("E00   0.0", "0 吨标定下限保护");
let i = 0;
while i < 11 {
    tap_key(S5, 80);
    settle();
    i += 1;
}
assert_display_exact("E05   1.0", "回到 5 吨标定点");
let j = 0;
while j < 10 {
    tap_key(S9, 80);
    settle();
    j += 1;
}
assert_display_exact("E05   0.0", "构造非法非单调标定");
tap_key(S4, 80);
settle();
assert_time_page(23, 59, 50, "非法标定退出后回到时间页");
goto_std_menu();
assert_display_exact("E00   0.0", "非法标定退出后应回滚");
tap_key(S5, 80);
settle();
assert_display_exact("E05   1.0", "非法标定回滚后 5 吨值恢复");
