run_ms(220);
assert_eq_str(display_text(), "       0", "上电无按键显示");
assert(!led_on(L1), "L1 初始应熄灭");
assert(!led_on(L8), "L8 初始应熄灭");

set_key(S4, true);
run_ms(220);
assert_eq_str(display_text(), "       1", "S4 位码显示");
assert(led_on(L1), "S4 按下后 L1 应翻转点亮");

set_key(S4, false);
run_ms(220);
assert_eq_str(display_text(), "       0", "释放 S4 后显示恢复");
assert(led_on(L1), "释放 S4 不应改变 L1");

set_key(S12, true);
run_ms(220);
assert_eq_str(display_text(), "     256", "S12 位码显示");
assert(!led_on(L1), "S12 按下后 L1 应再次翻转熄灭");

set_key(S12, false);
run_ms(220);
assert_eq_str(display_text(), "       0", "释放 S12 后显示恢复");

set_key(S14, true);
set_key(S19, true);
run_ms(220);
assert_eq_str(display_text(), "   33792", "组合按键位码显示");
assert(led_on(L3), "S14 按下后 L3 应点亮");
assert(led_on(L8), "S19 按下后 L8 应点亮");

set_key(S14, false);
set_key(S19, false);
run_ms(220);
assert_eq_str(display_text(), "       0", "释放组合按键后显示恢复");
