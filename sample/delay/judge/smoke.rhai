fn assert_all_off(label) {
    let led = 1;
    while led <= 8 {
        assert(!led_on(led), label + " 所有 LED 都应熄灭");
        led += 1;
    }
}

fn assert_only_led(expected, label) {
    let led = 1;
    while led <= 8 {
        if led == expected {
            assert(led_on(led), label + ", L" + led + " 应点亮");
        } else {
            assert(!led_on(led), label + ", 非 L" + led + " 应熄灭");
        }
        led += 1;
    }
}

run_us(34);
assert_all_off("上电早期");

let step = 0;
while step < 10 {
    run_ms(5);
    let expected = (step % 8) + 1;
    assert_only_led(expected, "流水灯步进 step: " + step);
    step += 1;
}

reset();
let stats = watch_led_stats(L1, 200);
assert_in(stats.changes, 9..=11, "200ms 内 L1 变化次数约 10");
assert_in(stats.rising_edges, 4..=6, "200ms 内 L1 上升沿约 5");
assert_in(stats.pwm_frequency_hz, 20..=30, "L1 循环频率约 25Hz");
assert_in(stats.duty_percent, 10..=15, "L1 占空比约 12.5%");
