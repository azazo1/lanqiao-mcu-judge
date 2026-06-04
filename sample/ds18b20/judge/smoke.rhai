add_marker("boot");
run_ms(700);
run_ms(30);
add_marker("boot_conversion_done");
assert_eq(display_number(1, 6), 0.000, "上电温度默认值");
assert_eq(display_number(8, 8), 0, "上电精度等级");

set_temperature_c(25.9375);
run_ms(700);
run_ms(30);
add_marker("resolution_9bit_sampled");
assert_eq(display_number(1, 6), 25.500, "9bit 温度量化结果");
assert_eq(display_number(8, 8), 0, "初始精度等级");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
assert_eq(display_number(1, 6), 25.750, "10bit 温度量化结果");
assert_eq(display_number(8, 8), 1, "S5 后精度等级");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
assert_eq(display_number(1, 6), 25.875, "11bit 温度量化结果");
assert_eq(display_number(8, 8), 2, "再次按 S5 后精度等级");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
add_marker("resolution_12bit_sampled");
assert_eq(display_number(1, 6), 25.937, "12bit 温度量化结果");
assert_eq(display_number(8, 8), 3, "第3次按 S5 后精度等级");

tap_key(S5, 80);
run_ms(700);
run_ms(30);
assert_eq(display_number(1, 6), 25.500, "精度等级回绕后温度量化结果");
assert_eq(display_number(8, 8), 0, "S5 回绕到 0");

tap_key(S4, 80);
run_ms(700);
run_ms(30);
assert_eq(display_number(1, 6), 25.937, "S4 反向回绕到 12bit");
assert_eq(display_number(8, 8), 3, "S4 从 0 回绕到 3");

set_temperature_c(-25);
run_ms(1100);
run_ms(30);
add_marker("temp_minus_25_sampled");
assert_eq(display_number(1, 6), -25.000, "-25 摄氏度显示");
assert_eq(display_number(8, 8), 3, "负温时保留当前精度等级");

set_temperature_c(100);
run_ms(960);
add_marker("temp_100_sampled");
assert_eq(display_number(1, 6), 100.000, "100 摄氏度显示");
assert_eq(display_number(8, 8), 3, "高温时保留当前精度等级");
