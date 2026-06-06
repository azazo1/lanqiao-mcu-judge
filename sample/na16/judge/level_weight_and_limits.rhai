key_mode(KEYBOARD);

fn sensor_settle() {
    run_ms(1600);
}

fn page_settle() {
    run_ms(160);
}

fn goto_liq_page() {
    tap_key(S5, 80);
    page_settle();
}

fn goto_weight_page() {
    goto_liq_page();
    tap_key(S5, 80);
    page_settle();
    tap_key(S5, 80);
    page_settle();
}

fn assert_data_page(temp, value_min, value_max, label) {
    let text = display_text(30);
    assert_regex(text, "^-?\\s?\\d{1,2}-\\s?\\d{1,2}\\.\\d{1,2}$", label + " 数据格式");
    assert_eq(display_number(1, 4), temp, label + " 左侧数值");
    let value = display_number(5, 8);
    assert(value >= value_min && value <= value_max, label + " 右侧数值");
}

set_temperature_c(25);
set_distance_cm(35);
sensor_settle();
goto_liq_page();
assert_data_page(25, 0.63, 0.66, "25C 35cm 液位");

reset(POWER_RESET);
set_temperature_c(48);
set_distance_cm(58);
sensor_settle();
goto_liq_page();
assert_data_page(48, 0.40, 0.44, "48C 58cm 液位");

reset(POWER_RESET);
set_temperature_c(78);
set_distance_cm(86);
sensor_settle();
goto_liq_page();
assert_data_page(78, 0.04, 0.06, "78C 86cm 液位");

reset(POWER_RESET);
set_temperature_c(-17);
set_distance_cm(65);
sensor_settle();
goto_liq_page();
assert_data_page(-17, 0.37, 0.40, "-17C 65cm 液位");

reset(POWER_RESET);
set_temperature_c(-38);
set_distance_cm(35);
sensor_settle();
goto_liq_page();
assert_data_page(-38, 0.67, 0.69, "-38C 35cm 液位");

reset(POWER_RESET);
set_temperature_c(25);
set_voltage(RB2, 2.3);
sensor_settle();
goto_weight_page();
assert_data_page(25, 10.6, 10.8, "25C 2.3V 重量");

reset(POWER_RESET);
set_temperature_c(48);
set_voltage(RB2, 2.3);
sensor_settle();
goto_weight_page();
assert_data_page(48, 10.2, 10.4, "48C 2.3V 重量");

reset(POWER_RESET);
set_temperature_c(78);
set_voltage(RB2, 3.5);
sensor_settle();
goto_weight_page();
assert_data_page(78, 12.8, 13.0, "78C 3.5V 重量");

reset(POWER_RESET);
set_temperature_c(-17);
set_voltage(RB2, 0.0);
sensor_settle();
goto_weight_page();
assert_data_page(-17, 1.8, 2.1, "-17C 0V 重量");

reset(POWER_RESET);
set_temperature_c(-38);
set_voltage(RB2, 4.5);
sensor_settle();
goto_weight_page();
assert_data_page(-38, 19.8, 20.0, "-38C 4.5V 重量上边界");

reset(POWER_RESET);
set_rtc(17, 45, 23);
set_distance_cm(170);
sensor_settle();
goto_liq_page();
assert_data_page(85, 0.25, 0.30, "170cm 低液位");
assert(relay_on(), "低于液位下限时继电器应吸合");
assert_eq(uart_take(), "[501,174523,0.30]", "501 异常串口上报");

reset(POWER_RESET);
set_rtc(17, 45, 23);
set_distance_cm(5);
sensor_settle();
goto_liq_page();
assert_data_page(85, 2.40, 2.45, "5cm 高液位");
assert(relay_on(), "超过液位上限时继电器应吸合");
assert_eq(uart_take(), "[500,174523,2.44]", "500 异常串口上报");
