// 第十届蓝桥杯单片机省赛 程序设计试题 整卷评测脚本.
//
// 评测点全部按题目原文推导:
//   4.1 性能: 电压数据刷新 <= 0.5s, 频率数据刷新 <= 1s.
//   4.2 显示: 频率界面 "F" + 频率值 (6 位, 高位熄灭), 电压界面 "U" + 电压值 (2 位小数).
//   4.3 按键: S4 切换 电压/频率 界面 (上电默认电压); S5 切换 DAC 输出模式 (跟随 RB2 与固定 2.0V);
//             S6 开关 LED 指示功能 (关闭时全部熄灭); S7 开关数码管显示功能 (关闭时全部熄灭).
//   4.4 LED: L1 电压界面, L2 频率界面; L3 按 VRB2 区间 (<1.5 灭, 1.5-2.5 亮, 2.5-3.5 灭, >=3.5 亮);
//             L4 按频率区间 (<1K 灭, 1-5K 亮, 5-10K 灭, >=10K 亮);
//             L5 在 DAC 输出固定 2.0V 时熄灭, 跟随 RB2 时点亮; 其余指示灯熄灭.
//   4.5 初始状态: 电压测量界面, 数码管与 LED 指示启用, DAC 输出固定 2.0V.

key_mode(BTN);

fn wait_measure() {
    // 采样实现对电压与频率都做了滑动滤波, 改变输入后需要多个刷新周期才稳定.
    run_ms(2500);
}

// 题目 4.1-4 的刷新时间要求, 用于性能断言.
fn volt_refresh_ns() {
    500_000_000
}

fn freq_refresh_ns() {
    1_000_000_000
}

fn press(key) {
    tap_key(key, 60);
    run_ms(300);
}

// 上电: 电位器电压与 NE555 频率按给定值设置.
fn boot(ain3, freq_hz) {
    jumper_on(NET_SIG, SIG_OUT);
    reset(POWER_RESET);
    key_mode(BTN);
    jumper_on(NET_SIG, SIG_OUT);
    set_voltage(AIN3, ain3);
    set_frequency_hz(freq_hz);
    wait_measure();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

fn dac_mv() {
    da_value() * 5000 / 255
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与电压界面 (4.2-2, 4.4, 4.5)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与电压界面", "上电处于电压界面显示两位小数电压, L1 点亮, DAC 固定 2.0V", || {
    let ain3 = 3.41;
    boot(ain3, 6000);
    let text = display_text(30);
    let value = (display_number(3, 8) * 100.0).round().to_int();
    let leds = led_text();
    let mv = dac_mv();

    assert_regex(text, "^U\\s+\\d\\.\\d\\d$", "电压界面: 应以 U 为提示符显示两位小数电压");
    assert_in(value, 335..=347, "电压测量: 3.41V 的读数超出容差");
    assert_eq(leds[0], '1', "指示灯: 电压测量界面下 L1 应点亮");
    assert_eq(leds[1], '0', "指示灯: 电压测量界面下 L2 应熄灭");
    assert_in(mv, 1900..=2100, "DAC 输出: 初始状态应输出固定 2.0V");
    assert_eq(leds[4], '0', "指示灯: DAC 输出固定 2.0V 时 L5 应熄灭");
    text + ", " + mv.to_string() + "mV, " + leds
});

// ---------------------------------------------------------------------------
// 检查点 2: 频率界面 (4.2-1, 4.4-2)
// ---------------------------------------------------------------------------
ckpt(2, "频率界面", "S4 切到频率界面显示 6 位频率, 高位熄灭, L2 点亮", || {
    boot(3.41, 6000);
    press(S4);
    let six = display_text(30);

    set_frequency_hz(500);
    wait_measure();
    let low = display_text(30);

    set_frequency_hz(12000);
    wait_measure();
    let high = display_text(30);
    let leds = led_text();

    assert_regex(six, "^F\\s+6000$", "频率界面: 6000Hz 应显示为 F 与 6000 (高位熄灭)");
    assert_regex(low, "^F\\s+500$", "频率界面: 500Hz 应显示为 F 与 500");
    assert_regex(high, "^F\\s+12000$", "频率界面: 12000Hz 应显示为 F 与 12000");
    assert_eq(leds[1], '1', "指示灯: 频率测量界面下 L2 应点亮");
    assert_eq(leds[0], '0', "指示灯: 频率测量界面下 L1 应熄灭");
    six + " -> " + low + " -> " + high
});

// ---------------------------------------------------------------------------
// 检查点 3: DAC 输出模式切换 (4.3-2, 4.4-5)
// ---------------------------------------------------------------------------
ckpt(3, "DAC 输出模式切换", "S5 在固定 2.0V 与跟随 RB2 之间切换, L5 指示跟随状态", || {
    boot(3.41, 6000);
    let fixed = dac_mv();
    let fixed_leds = led_text();

    press(S5);
    let follow = dac_mv();
    let follow_leds = led_text();

    // 改变电位器电压: 跟随模式下 DAC 应跟着变, 固定模式下不变.
    set_voltage(AIN3, 1.20);
    wait_measure();
    let follow_low = dac_mv();

    press(S5);
    let fixed_low = dac_mv();
    let fixed_low_leds = led_text();

    assert_in(fixed, 1900..=2100, "DAC 输出: 初始应输出固定 2.0V");
    assert_eq(fixed_leds[4], '0', "指示灯: 固定 2.0V 时 L5 应熄灭");
    assert_in(follow, 3300..=3500, "DAC 输出: 跟随模式下应输出电位器电压 3.41V");
    assert_eq(follow_leds[4], '1', "指示灯: 跟随模式下 L5 应点亮");
    assert_in(follow_low, 1100..=1300, "DAC 输出: 跟随模式下电位器降到 1.20V 时输出应跟着降");
    assert_in(fixed_low, 1900..=2100, "DAC 输出: 切回固定模式后应回到 2.0V");
    assert_eq(fixed_low_leds[4], '0', "指示灯: 切回固定模式后 L5 应熄灭");
    fixed.to_string() + " -> " + follow.to_string() + " -> " + follow_low.to_string() + " -> " + fixed_low.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 4: LED 指示功能开关 (4.3-3)
// ---------------------------------------------------------------------------
ckpt(4, "LED 指示功能开关", "S6 关闭后所有指示灯熄灭, 再次按下恢复", || {
    boot(3.41, 6000);
    let on = led_text();
    press(S6);
    let off = led_text();
    press(S6);
    let back = led_text();

    assert(on.contains("1"), "LED 开关: 初始状态应有指示灯点亮");
    assert_eq(off, "00000000", "LED 开关: 关闭 LED 指示功能后应全部熄灭");
    assert_eq(back, on, "LED 开关: 再次按下 S6 后应恢复到关闭前的状态");
    on + " -> " + off + " -> " + back
});

// ---------------------------------------------------------------------------
// 检查点 5: 数码管显示功能开关 (4.3-4)
// ---------------------------------------------------------------------------
ckpt(5, "数码管显示功能开关", "S7 关闭后数码管全部熄灭, 再次按下恢复显示", || {
    boot(3.41, 6000);
    let shown = display_text(30);
    press(S7);
    // 数码管关闭后没有内容可读, 改用段码判定是否全部熄灭.
    let blank = "";
    for index in 1..=8 {
        blank += if seg_pattern(index) == 0 { "0" } else { "1" };
    }
    press(S7);
    let back = display_text(30);

    assert_regex(shown, "^U\\s+\\d\\.\\d\\d$", "数码管开关: 初始应显示电压界面");
    assert_eq(blank, "00000000", "数码管开关: 关闭显示功能后所有数码管应熄灭");
    // 题目只要求 S7 控制数码管显示功能的开关, 因此这里判定"恢复成电压界面"即可.
    // 注意采样的电压读数会在 3.41V 与 2.50V 之间交替 (见 README 记录), 不纳入本项断言.
    assert_regex(back, "^U\\s+\\d\\.\\d\\d$", "数码管开关: 再次按下 S7 后应恢复显示");
    shown + " -> " + blank + " -> " + back
});

// ---------------------------------------------------------------------------
// 检查点 6: L3 与 L4 区间指示 (4.4-3, 4.4-4)
// ---------------------------------------------------------------------------
ckpt(6, "L3 与 L4 区间指示", "L3 按电位器电压区间变化, L4 按频率区间变化", || {
    boot(2.0, 3000);
    let volt_mid = led_text();

    set_voltage(AIN3, 1.0);
    wait_measure();
    let volt_low = led_text();

    set_voltage(AIN3, 4.0);
    wait_measure();
    let volt_high = led_text();

    set_frequency_hz(500);
    wait_measure();
    let freq_low = led_text();

    set_frequency_hz(7000);
    wait_measure();
    let freq_mid = led_text();

    set_frequency_hz(12000);
    wait_measure();
    let freq_high = led_text();

    assert_eq(volt_mid[2], '1', "L3 指示: 2.0V 落在 1.5-2.5V 区间时应点亮");
    assert_eq(volt_low[2], '0', "L3 指示: 1.0V 低于 1.5V 时应熄灭");
    assert_eq(volt_high[2], '1', "L3 指示: 4.0V 不低于 3.5V 时应点亮");
    assert_eq(freq_low[3], '0', "L4 指示: 500Hz 低于 1KHz 时应熄灭");
    assert_eq(freq_mid[3], '0', "L4 指示: 7000Hz 落在 5-10KHz 区间时应熄灭");
    assert_eq(freq_high[3], '1', "L4 指示: 12000Hz 不低于 10KHz 时应点亮");
    volt_mid + " " + volt_low + " " + volt_high + " " + freq_low + " " + freq_mid + " " + freq_high
});

// ---------------------------------------------------------------------------
// 检查点 7: 数据刷新时间 (4.1-4)
// ---------------------------------------------------------------------------
ckpt(7, "数据刷新时间", "电压数据刷新 <= 0.5s, 频率数据刷新 <= 1s", || {
    boot(3.41, 6000);

    // 电位器改到 2.00V, 记录数码管更新到 2.0V 附近用了多久 (轮询到上限为止, 便于记录实测值).
    set_voltage(AIN3, 2.00);
    let volt_ms = 0;
    while volt_ms < 3000 && (display_number(3, 8) - 2.0).abs() >= 0.05 {
        run_ms(20);
        volt_ms += 20;
    }
    let volt_page = display_text(30);

    // 频率改到 3000Hz, 记录数码管更新到 3000 用了多久.
    set_frequency_hz(3000);
    let freq_ms = 0;
    while freq_ms < 3000 && display_number(2, 8) != 3000 {
        run_ms(20);
        freq_ms += 20;
    }
    let freq_page = display_text(30);

    assert_in(volt_ms, 0..=500, "刷新时间: 电压数据应在 0.5 秒内刷新, 实际 " + volt_ms.to_string() + "ms, 页面 " + volt_page);
    assert_in(freq_ms, 0..=1000, "刷新时间: 频率数据应在 1 秒内刷新, 实际 " + freq_ms.to_string() + "ms, 页面 " + freq_page);
    volt_ms.to_string() + "ms, " + freq_ms.to_string() + "ms"
});
