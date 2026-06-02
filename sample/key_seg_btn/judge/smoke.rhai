key_mode(BUTTON);
run_ms(220);
assert_eq_str(display_text(), "       0", "上电无按键显示");
assert(!led_on(L1), "L1 初始应熄灭");
assert(!led_on(L2), "L2 初始应熄灭");
assert(!led_on(L3), "L3 初始应熄灭");
assert(!led_on(L4), "L4 初始应熄灭");

set_key(S4, true);
run_ms(220);
assert_eq_str(display_text(30), "       1", "S4 独立按键显示");
assert(led_on(L1), "S4 按下后 L1 应翻转点亮");

set_key(S4, false);
run_ms(220);
assert_eq_str(display_text(30), "       0", "释放 S4 后显示恢复");
assert(led_on(L1), "释放 S4 不应改变 L1");

set_key(S5, true);
run_ms(220);
assert_eq_str(display_text(30), "       2", "S5 独立按键显示");
assert(led_on(L2), "S5 按下后 L2 应翻转点亮");

set_key(S5, false);
run_ms(220);
assert_eq_str(display_text(30), "       0", "释放 S5 后显示恢复");

set_key(S4, true);
set_key(S7, true);
run_ms(220);
assert_eq_str(display_text(30), "       9", "S4 和 S7 组合位码显示");
assert(!led_on(L1), "S4 第二次按下后 L1 应翻转熄灭");
assert(led_on(L4), "S7 按下后 L4 应翻转点亮");

set_key(S4, false);
set_key(S7, false);
run_ms(220);
assert_eq_str(display_text(30), "       0", "释放组合按键后显示恢复");
