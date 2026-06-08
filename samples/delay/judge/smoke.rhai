fn assert_all_off(label) {
    let led = 1;
    while led <= 8 {
        assert(!led_on(led), label + ": LED 熄灭状态错误, L" + led + " 实际点亮");
        led += 1;
    }
}

fn assert_only_led(expected, label) {
    let led = 1;
    while led <= 8 {
        if led == expected {
            assert(led_on(led), label + ": LED 点亮状态错误, 期望 L" + led + " 点亮, 实际未点亮");
        } else {
            assert(!led_on(led), label + ": LED 熄灭状态错误, 期望非 L" + expected + " 熄灭, L" + led + " 实际点亮");
        }
        led += 1;
    }
}

fn assert_step_delay_ns(dt_ns, label) {
    assert_in(dt_ns, 4_500_000..=5_500_000, label + ": Delay5ms 耗时超出范围");
}

fn all_off() {
    let led = 1;
    while led <= 8 {
        if led_on(led) {
            return false;
        }
        led += 1;
    }
    true
}

fn wait_led_on(target, expected, label) {
    let dt_ns = run_to(target, UP, 20_000_000);
    assert_step_delay_ns(dt_ns, label);
    assert_only_led(expected, label);
}

let startup_ns = run_to(|| all_off(), 2_000_000);
assert_in(startup_ns, 0..=200_000, "上电初始化: LED 清空时延超出范围");
assert_all_off("上电早期");

wait_led_on(L1, 1, "流水灯步进 step: 0");
wait_led_on(L2, 2, "流水灯步进 step: 1");
wait_led_on(L3, 3, "流水灯步进 step: 2");
wait_led_on(L4, 4, "流水灯步进 step: 3");
wait_led_on(L5, 5, "流水灯步进 step: 4");
wait_led_on(L6, 6, "流水灯步进 step: 5");
wait_led_on(L7, 7, "流水灯步进 step: 6");
wait_led_on(L8, 8, "流水灯步进 step: 7");
wait_led_on(L1, 1, "流水灯步进 step: 8");
wait_led_on(L2, 2, "流水灯步进 step: 9");
