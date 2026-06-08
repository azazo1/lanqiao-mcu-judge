add_marker("boot");
run_ms(700);
run_ms(30);
add_marker("boot_conversion_done");
assert_eq(display_number(1, 6), 0.000, "上电检查: 温度默认值错误");
assert_eq(display_number(8, 8), 0, "上电检查: 精度等级错误");

set_temperature_c(25.9375);
run_ms(700);
run_ms(30);
add_marker("resolution_9bit_sampled");
assert_eq(display_number(1, 6), 25.500, "9bit 采样: 温度量化结果错误");
assert_eq(display_number(8, 8), 0, "9bit 采样: 精度等级错误");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
assert_eq(display_number(1, 6), 25.750, "10bit 采样: 温度量化结果错误");
assert_eq(display_number(8, 8), 1, "10bit 采样: 精度等级错误");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
assert_eq(display_number(1, 6), 25.875, "11bit 采样: 温度量化结果错误");
assert_eq(display_number(8, 8), 2, "11bit 采样: 精度等级错误");

tap_key(S5, 80);
run_ms(400);
run_ms(30);
add_marker("resolution_12bit_sampled");
assert_eq(display_number(1, 6), 25.937, "12bit 采样: 温度量化结果错误");
assert_eq(display_number(8, 8), 3, "12bit 采样: 精度等级错误");

tap_key(S5, 80);
run_ms(700);
run_ms(30);
assert_eq(display_number(1, 6), 25.500, "精度回绕: 温度量化结果错误");
assert_eq(display_number(8, 8), 0, "精度回绕: 精度等级错误");

tap_key(S4, 80);
run_ms(700);
run_ms(30);
assert_eq(display_number(1, 6), 25.937, "反向回绕: 温度量化结果错误");
assert_eq(display_number(8, 8), 3, "反向回绕: 精度等级错误");

set_temperature_c(-25);
run_ms(1100);
run_ms(30);
add_marker("temp_minus_25_sampled");
assert_eq(display_number(1, 6), -25.000, "-25C 采样: 温度显示错误");
assert_eq(display_number(8, 8), 3, "-25C 采样: 精度等级错误");

set_temperature_c(100);
run_ms(960);
add_marker("temp_100_sampled");
assert_eq(display_number(1, 6), 100.000, "100C 采样: 温度显示错误");
assert_eq(display_number(8, 8), 3, "100C 采样: 精度等级错误");
