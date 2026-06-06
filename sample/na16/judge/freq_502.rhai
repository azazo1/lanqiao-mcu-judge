jumper_on(NET_SIG, SIG_OUT);
run_ms(5);
set_rtc(22, 18, 42);
run_ms(320);
set_frequency_hz(2200);
run_ms(1300);

assert_eq(display_text(), "22-18-42", "502 场景: 时间显示错误");
assert(led_on(L1), "502 场景: L1 状态错误, 期望点亮, 实际未点亮");
assert(!led_on(L2), "502 场景: L2 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L3), "502 场景: L3 状态错误, 期望熄灭, 实际点亮");
assert(!led_on(L4), "502 场景: L4 状态错误, 期望熄灭, 实际点亮");
assert(led_on(L6), "502 场景: L6 状态错误, 期望点亮, 实际未点亮");
assert(relay_on(), "502 场景: 继电器状态错误, 期望吸合, 实际断开");
assert_eq(uart_take(), "[502,221842,2200]", "502 场景: 串口上报错误");
