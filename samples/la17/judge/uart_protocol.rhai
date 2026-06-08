key_mode(KEYBOARD);

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
set_eeprom([0x16, 0x71, 0x0B, 40, 144, 0x8E]);
set_voltage(AIN1, 0.0);
set_voltage(AIN3, 0.0);
set_distance_cm(0);
set_frequency_hz(0);
reset(POWER_RESET);
run_ms(320);

assert_eq(display_text(30), "11-59-50", "上电恢复: 12h 时间显示错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:12|TP:1|LM:1|SV:360|DAC:144;", "配置查询: EEPROM 恢复配置错误");

uart_write("$SET|DAC:200|LM:0|CLK:24|SV:340|TP:3;");
run_ms(220);
assert_eq(uart_take(), "$OK|SET;", "多字段配置: 回复错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:24|TP:3|LM:0|SV:340|DAC:200;", "配置查询: 修改后配置错误");
assert_eq(display_text(30), "23-59-50", "切回 24h: 时间显示错误");

uart_write("$SET|DAC:255;");
run_ms(220);
assert_eq(uart_take(), "$OK|SET;", "DAC 上界更新: 回复错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:24|TP:3|LM:0|SV:340|DAC:255;", "DAC 上界更新: 配置保持错误");

uart_write("$SET|LM:1|DAC:000;");
run_ms(220);
assert_eq(uart_take(), "$OK|SET;", "乱序更新: 回复错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:24|TP:3|LM:1|SV:340|DAC:000;", "乱序更新: 配置结果错误");

uart_write("$SET|TP:0;");
run_ms(220);
assert_eq(uart_take(), "$OK|SET;", "温度精度更新: 回复错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:24|TP:0|LM:1|SV:340|DAC:000;", "温度精度更新: 配置结果错误");

uart_write("$SET|SV:379;");
run_ms(220);
assert_eq(uart_take(), "$OK|SET;", "音速上界更新: 回复错误");

uart_write("$Q|CFG;");
run_ms(220);
assert_eq(uart_take(), "$CFG|CLK:24|TP:0|LM:1|SV:379|DAC:000;", "音速上界更新: 配置结果错误");

uart_write("$RTC|DATE:26-12-31|TIME:23:58:50;");
run_ms(220);
assert_eq(uart_take(), "$OK|RTC;", "RTC 设置: 回复错误");
assert_eq(display_text(30), "23-58-50", "RTC 设置: 时间显示错误");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "26-12-31", "S5 切页: 日期页显示错误");

tap_key(S5, 80);
run_ms(120);
goto_temp_page();
set_temperature_c(-5.125);
run_ms(260);
assert_eq(display_text(30), "-5.000-0", "环境查询前: 负温显示错误");

uart_write("$Q|ENV;");
run_ms(220);
assert_eq(
    uart_take(),
    "$ENV|TMP:-5.000|L:000|VR:000|DIST:000|FREQ:0000;",
    "环境查询: 返回内容错误"
);

uart_write("$SET|DAC:256;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法 DAC: 回复错误");

uart_write("$SET|SV:319;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法音速下界: 回复错误");

uart_write("$SET|SV:400;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法音速上界: 回复错误");

uart_write("$SET|LM:2;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法模式字段: 回复错误");

uart_write("$SET|TP:4;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法精度字段: 回复错误");

uart_write("$SET|ABC:1;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法字段名: 回复错误");

uart_write("$RTC|DATE:26-02-30|TIME:12:00:00;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法 RTC 日期: 回复错误");
assert_eq(display_text(30), "-5.000-0", "非法 RTC 日期: 当前页面显示被改坏");

uart_write("$RTC|DATE:26-12-31|TIME:24:00:00;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法 RTC 小时: 回复错误");
assert_eq(display_text(30), "-5.000-0", "非法 RTC 小时: 当前页面显示被改坏");

uart_write("$RTC|DATE:26-12-31|TIME:23:60:00;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法 RTC 分钟: 回复错误");
assert_eq(display_text(30), "-5.000-0", "非法 RTC 分钟: 当前页面显示被改坏");

uart_write("$RTC|DATE:26-12-31|TIME:23:59:60;");
run_ms(220);
assert_eq(uart_take(), "$ERR|VALUE;", "非法 RTC 秒钟: 回复错误");
assert_eq(display_text(30), "-5.000-0", "非法 RTC 秒钟: 当前页面显示被改坏");

uart_write("SET|CLK:12;");
run_ms(220);
assert_eq(uart_take(), "$ERR|FRAME;", "非法帧格式: 回复错误");
