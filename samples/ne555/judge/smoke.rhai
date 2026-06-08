run_ms(500);

assert_eq(parse_int(display_text()[0..3]), 0, "上电检查: 按键区显示错误");
assert_eq(parse_int(display_text()[3..8]), 0, "未接跳帽: 频率显示错误");

set_frequency_hz(500);
run_ms(1100);
assert_eq(parse_int(display_text()[3..8]), 0, "未接跳帽设置500Hz: 频率显示错误");

jumper_on(NET_SIG, SIG_OUT);
run_ms(2200);
let freq_500 = parse_int(display_text()[3..8]);
assert_in(freq_500, 490..=510, "500Hz 测频: 频率结果超出范围");

set_frequency_hz(2200);
run_ms(2200);
let freq_2200 = parse_int(display_text()[3..8]);
assert_in(freq_2200, 2180..=2220, "2200Hz 测频: 频率结果超出范围");

jumper_off(NET_SIG, SIG_OUT);
run_ms(2200);
assert_eq(parse_int(display_text()[3..8]), 0, "断开跳帽: 频率回零错误");
