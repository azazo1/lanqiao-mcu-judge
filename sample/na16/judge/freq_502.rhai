jumper_on(NET_SIG, SIG_OUT);
run_ms(5);
set_rtc(22, 18, 42);
run_ms(320);
set_frequency_hz(2200);
run_ms(1300);

assert_eq(display_text(), "22-18-42", "502 场景下时间显示应保持");
assert(led_on(L1), "502 场景下 L1 应点亮");
assert(!led_on(L2), "502 场景下 L2 应熄灭");
assert(!led_on(L3), "502 场景下 L3 应熄灭");
assert(!led_on(L4), "502 场景下 L4 应熄灭");
assert(led_on(L6), "502 场景下 L6 应点亮");
assert(relay_on(), "502 场景下继电器应吸合");
assert_eq(uart_take(), "[502,221842,2200]", "502 场景下串口应立即上报");
