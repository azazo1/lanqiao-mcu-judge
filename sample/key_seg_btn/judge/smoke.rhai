key_mode(BUTTON);
run_ms(220);
assert(parse_int(display_text()) == 0, "上电末位数值");
assert(!led_on(L1), "L1 初始应熄灭");
assert(!led_on(L2), "L2 初始应熄灭");
assert(!led_on(L3), "L3 初始应熄灭");
assert(!led_on(L4), "L4 初始应熄灭");

set_key(S4, true);
run_ms(220);
assert(parse_int(display_text()) == 1, "S4 独立按键数值");
assert(led_on(L1), "S4 按下后 L1 应翻转点亮");

set_key(S4, false);
run_ms(220);
assert(parse_int(display_text()) == 0, "释放 S4 后数值恢复");
assert(led_on(L1), "释放 S4 不应改变 L1");

set_key(S5, true);
run_ms(220);
assert(parse_int(display_text()) == 2, "S5 独立按键数值");
assert(led_on(L2), "S5 按下后 L2 应翻转点亮");

set_key(S5, false);
run_ms(220);
assert(parse_int(display_text()) == 0, "释放 S5 后数值恢复");

set_key(S4, true);
set_key(S7, true);
run_ms(220);
assert(parse_int(display_text()) == 9, "组合按键位码数值");
assert(!led_on(L1), "S4 第二次按下后 L1 应翻转熄灭");
assert(led_on(L4), "S7 按下后 L4 应翻转点亮");

set_key(S4, false);
set_key(S7, false);
run_ms(220);
assert(parse_int(display_text()) == 0, "释放组合按键后数值恢复");
