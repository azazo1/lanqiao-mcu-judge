fn settle() {
    run_ms(320);
}

fn assert_leds_for_504(label) {
    assert(led_on(L1), label + " L1");
    assert(!led_on(L2), label + " L2");
    assert(!led_on(L3), label + " L3");
    assert(!led_on(L4), label + " L4");
}

jumper_on(NET_SIG, SIG_OUT);

run_ms(5);
set_rtc(3, 44, 5);
run_ms(320);
set_frequency_hz(400);
run_ms(1200);
set_frequency_hz(1700);
run_ms(1200);
assert_eq(uart_take(), "[504,034405,1300]", "504 异常串口上报");
assert(relay_on(), "504 异常上报后继电器应吸合");
assert_leds_for_504("504 异常上报后页面指示");
let blink_504 = watch_led_stats(L8, 1000);
assert_in(blink_504.changes, 4..=6, "504 异常时 L8 应以 0.2s 间隔闪烁");

reset(POWER_RESET);
run_ms(320);
jumper_on(NET_SIG, SIG_OUT);
set_rtc(22, 18, 42);
run_ms(320);
set_frequency_hz(2200);
run_ms(2100);
assert_eq(uart_take(), "[502,221842,2200]", "502 异常串口上报");
assert(led_on(L6), "502 异常时 L6 应点亮");
assert(relay_on(), "502 异常时继电器应吸合");
assert(!led_on(L7), "仅 502 异常时 L7 应熄灭");

reset(POWER_RESET);
run_ms(320);
jumper_on(NET_SIG, SIG_OUT);
set_rtc(23, 33, 44);
run_ms(320);
set_frequency_hz(1500);
run_ms(1200);
set_frequency_hz(1300);
run_ms(1200);
set_frequency_hz(1000);
run_ms(1200);
set_frequency_hz(800);
run_ms(1200);
set_frequency_hz(500);
run_ms(1200);
assert_eq(uart_take(), "[503,233344,0]", "503 异常串口上报");
assert(relay_on(), "503 异常时继电器应吸合");
let blink_503 = watch_led_stats(L7, 1000);
assert_in(blink_503.changes, 9..=11, "503 异常时 L7 应以 0.1s 间隔闪烁");
run_ms(2200);
assert_eq(uart_take(), "", "503 持续期间不应重复上报");
