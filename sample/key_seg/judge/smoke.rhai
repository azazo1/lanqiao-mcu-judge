run_ms(220);
assert(regex_is_match(display_text(30), "^\\s{7}0$"), "上电显示格式");
assert_eq_str(display_text(30)[0..7], "       ", "上电前7位空白");
assert(parse_int(display_text()[7..8]) == 0, "上电末位数值");
assert(!led_on(L1), "L1 初始应熄灭");
assert(!led_on(L8), "L8 初始应熄灭");

set_key(S4, true);
run_ms(220);
assert_eq_str(display_text(30)[0..7], "       ", "S4 前7位空白");
assert(parse_int(display_text()[7..8]) == 1, "S4 位码数值");
assert(led_on(L1), "S4 按下后 L1 应翻转点亮");

set_key(S4, false);
run_ms(220);
assert_eq_str(display_text(30)[0..7], "       ", "释放 S4 后前7位空白");
assert(parse_int(display_text()[7..8]) == 0, "释放 S4 后数值恢复");
assert(led_on(L1), "释放 S4 不应改变 L1");

set_key(S12, true);
run_ms(220);
assert_eq_str(display_text(30)[0..5], "     ", "S12 前5位空白");
assert(parse_int(display_text()[5..8]) == 256, "S12 位码数值");
assert(!led_on(L1), "S12 按下后 L1 应再次翻转熄灭");

set_key(S12, false);
run_ms(220);
assert_eq_str(display_text(30)[0..7], "       ", "释放 S12 后前7位空白");
assert(parse_int(display_text()[7..8]) == 0, "释放 S12 后数值恢复");

set_key(S14, true);
set_key(S19, true);
run_ms(220);
assert_eq_str(display_text(30)[0..3], "   ", "组合按键前3位空白");
assert(parse_int(display_text()[3..8]) == 33792, "组合按键位码数值");
assert(led_on(L3), "S14 按下后 L3 应点亮");
assert(led_on(L8), "S19 按下后 L8 应点亮");

set_key(S14, false);
set_key(S19, false);
run_ms(220);
assert_eq_str(display_text(30)[0..7], "       ", "释放组合按键后前7位空白");
assert(parse_int(display_text()[7..8]) == 0, "释放组合按键后数值恢复");
