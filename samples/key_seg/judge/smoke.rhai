run_ms(220);
assert_regex(display_text(30), "^\\s{7}0$", "上电检查: 显示格式错误");
assert_eq(display_text(30)[0..7], "       ", "上电检查: 前7位空白显示错误");
assert_eq(parse_int(display_text()[7..8]), 0, "上电检查: 末位数值错误");
assert(!led_on(L1), "上电检查: L1 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L8), "上电检查: L8 状态错误, 期望熄灭, 实际点亮");

set_key(S4, true);
run_ms(220);
assert_eq(display_text(30)[0..7], "       ", "S4 按下: 前7位空白显示错误");
assert_eq(parse_int(display_text()[7..8]), 1, "S4 按下: 位码数值错误");
assert(led_on(L1), "S4 按下: L1 翻转结果错误, 期望点亮, 实际未点亮");

set_key(S4, false);
run_ms(220);
assert_eq(display_text(30)[0..7], "       ", "释放 S4: 前7位空白显示错误");
assert_eq(parse_int(display_text()[7..8]), 0, "释放 S4: 数值恢复错误");
assert(led_on(L1), "释放 S4: L1 保持状态错误, 期望保持点亮, 实际未点亮");

set_key(S12, true);
run_ms(220);
assert_eq(display_text(30)[0..5], "     ", "S12 按下: 前5位空白显示错误");
assert_eq(parse_int(display_text()[5..8]), 256, "S12 按下: 位码数值错误");
assert(!led_on(L1), "S12 按下: L1 翻转结果错误, 期望熄灭, 实际点亮");

set_key(S12, false);
run_ms(220);
assert_eq(display_text(30)[0..7], "       ", "释放 S12: 前7位空白显示错误");
assert_eq(parse_int(display_text()[7..8]), 0, "释放 S12: 数值恢复错误");

set_key(S14, true);
set_key(S19, true);
run_ms(220);
assert_eq(display_text(30)[0..3], "   ", "组合按键: 前3位空白显示错误");
assert_eq(parse_int(display_text()[3..8]), 33792, "组合按键: 位码数值错误");
assert(led_on(L3), "组合按键: L3 状态错误, 期望点亮, 实际未点亮");
assert(led_on(L8), "组合按键: L8 状态错误, 期望点亮, 实际未点亮");

set_key(S14, false);
set_key(S19, false);
run_ms(220);
assert_eq(display_text(30)[0..7], "       ", "释放组合按键: 前7位空白显示错误");
assert_eq(parse_int(display_text()[7..8]), 0, "释放组合按键: 数值恢复错误");
