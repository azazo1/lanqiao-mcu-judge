key_mode(KEYBOARD);

fn double_click(key) {
    tap_key(key, 80);
    run_ms(120);
    tap_key(key, 80);
    run_ms(220);
}

fn goto_temp_page() {
    tap_key(S4, 80);
    run_ms(120);
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
set_eeprom([0xA5, 0x5A, 0x00, 40, 128, 0xA7]);
set_voltage(AIN1, 0.0);
set_voltage(AIN3, 0.0);
set_distance_cm(0);
reset(POWER_RESET);
run_ms(320);

uart_write("@Q,CFG#");
run_ms(220);
assert_eq(uart_take(), "@CFG,24,0,340,128#", "查询默认配置");

uart_write("@CFG,DAC=180,SPD=347,PRE=3,MODE=12#");
run_ms(220);
assert_eq(uart_take(), "@OK,CFG#", "修改配置帧");

uart_write("@Q,CFG#");
run_ms(220);
assert_eq(uart_take(), "@CFG,12,3,347,180#", "查询修改后的配置");

double_click(S6);
double_click(S6);
assert_eq(display_text(30), "11-59-50", "配置切到 12h 后的时间显示");

uart_write("@RTC,DATE=26-12-31,TIME=23:58:50#");
run_ms(220);
assert_eq(uart_take(), "@OK,RTC#", "RTC 设置帧");
assert_eq(display_text(30), "11-58-50", "RTC 设置后的 12h 时间");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "26-12-31", "S5 后日期页显示");

tap_key(S5, 80);
run_ms(120);
goto_temp_page();
set_temperature_c(-5.125);
run_ms(260);
assert_eq(display_text(30), "-5.125-3", "PRE=3 下的负温显示");

reset(POWER_RESET);
run_ms(320);
uart_write("@Q,ALL#");
run_ms(220);
assert_eq(
    uart_take(),
    "@ALL,26-12-31,11-58-50,12,3,347,180,000,000,000,0000#",
    "查询全部状态"
);

uart_write("@CFG,MODE=13#");
run_ms(220);
assert_eq(uart_take(), "@ERR,VALUE#", "非法配置值");

uart_write("CFG,MODE=12#");
run_ms(220);
assert_eq(uart_take(), "@ERR,FRAME#", "非法帧格式");
