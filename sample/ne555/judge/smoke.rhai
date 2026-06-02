run_ms(500);

assert(parse_int(display_text()[0..3]) == 0, "上电按键区默认显示 000");
assert(parse_int(display_text()[3..8]) == 0, "未接跳帽时频率应为 0");

set_frequency_hz(500);
run_ms(1100);
assert(parse_int(display_text()[3..8]) == 0, "仅设置频率但未接跳帽时仍应为 0");

jumper_on(NET_SIG, SIG_OUT);
run_ms(1200);
let freq_500 = parse_int(display_text()[3..8]);
assert(freq_500 >= 490 && freq_500 <= 510, "500Hz 测频结果应落在允许范围内");

set_frequency_hz(2200);
run_ms(1200);
let freq_2200 = parse_int(display_text()[3..8]);
assert(freq_2200 >= 2180 && freq_2200 <= 2220, "2200Hz 测频结果应落在允许范围内");

jumper_off(NET_SIG, SIG_OUT);
run_ms(1200);
assert(parse_int(display_text()[3..8]) == 0, "断开跳帽后频率应回到 0");
