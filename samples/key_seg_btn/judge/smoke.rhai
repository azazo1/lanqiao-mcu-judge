key_mode(BUTTON);
run_ms(220);
assert_eq(parse_int(display_text()), 0, "上电检查: 末位数值错误");
assert(!led_on(L1), "上电检查: L1 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L2), "上电检查: L2 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L3), "上电检查: L3 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L4), "上电检查: L4 状态错误, 期望熄灭, 实际点亮");

set_key(S4, true);
run_ms(220);
assert_eq(parse_int(display_text()), 1, "S4 按下: 按键数值错误");
assert(led_on(L1), "S4 按下: L1 翻转结果错误, 期望点亮, 实际未点亮");

set_key(S4, false);
run_ms(220);
assert_eq(parse_int(display_text()), 0, "释放 S4: 数值恢复错误");
assert(led_on(L1), "释放 S4: L1 保持状态错误, 期望保持点亮, 实际未点亮");

set_key(S5, true);
run_ms(220);
assert_eq(parse_int(display_text()), 2, "S5 按下: 按键数值错误");
assert(led_on(L2), "S5 按下: L2 翻转结果错误, 期望点亮, 实际未点亮");

set_key(S5, false);
run_ms(220);
assert_eq(parse_int(display_text()), 0, "释放 S5: 数值恢复错误");

set_key(S4, true);
set_key(S7, true);
run_ms(220);
assert_eq(parse_int(display_text()), 9, "组合按键: 位码数值错误");
assert(!led_on(L1), "组合按键: L1 翻转结果错误, 期望熄灭, 实际点亮");
assert(led_on(L4), "组合按键: L4 翻转结果错误, 期望点亮, 实际未点亮");

set_key(S4, false);
set_key(S7, false);
run_ms(220);
assert_eq(parse_int(display_text()), 0, "释放组合按键: 数值恢复错误");
