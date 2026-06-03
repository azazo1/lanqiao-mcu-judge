run_ms(700);
assert(regex_is_match(display_text(30), "^\\s{2}\\d{4} \\d$"), "上电显示格式");
assert(display_number(1, 6) == 0, "上电温度默认值");
assert(display_number(8, 8) == 0, "上电精度等级");

set_temperature_c(25.9375);
run_ms(700);
assert(display_number(1, 6) == 25500, "9bit 温度量化结果");
assert(display_number(8, 8) == 0, "初始精度等级");

tap_key(S5, 80);
run_ms(400);
assert(display_number(1, 6) == 25750, "10bit 温度量化结果");
assert(display_number(8, 8) == 1, "S5 后精度等级");

tap_key(S5, 80);
run_ms(400);
assert(display_number(1, 6) == 25875, "11bit 温度量化结果");
assert(display_number(8, 8) == 2, "再次按 S5 后精度等级");

tap_key(S5, 80);
run_ms(400);
assert(display_number(1, 6) == 25937, "12bit 温度量化结果");
assert(display_number(8, 8) == 3, "第3次按 S5 后精度等级");

tap_key(S5, 80);
run_ms(400);
assert(display_number(1, 6) == 25500, "精度等级回绕后温度量化结果");
assert(display_number(8, 8) == 0, "S5 回绕到 0");

tap_key(S4, 80);
run_ms(400);
assert(display_number(1, 6) == 25937, "S4 反向回绕到 12bit");
assert(display_number(8, 8) == 3, "S4 从 0 回绕到 3");

set_temperature_c(-25);
run_ms(700);
assert(display_number(1, 6) == -25000, "-25 摄氏度显示");
assert(display_number(8, 8) == 3, "负温时保留当前精度等级");

set_temperature_c(100);
run_ms(700);
assert(display_number(1, 6) == 100000, "100 摄氏度显示");
assert(display_number(8, 8) == 3, "高温时保留当前精度等级");
