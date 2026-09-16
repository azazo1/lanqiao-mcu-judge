// 第十三届蓝桥杯单片机省赛第二场 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.2 性能: 测距精度 <= ±3cm, 按键响应 <= 0.2s, 指示灯动作响应 <= 0.1s.
//   4.3 显示: 电压界面 "U" + 3 位数码管两位小数电压,
//             参数界面 "P" + 上限参数 + 下限参数 (各 2 位数码管, 1 位小数),
//             测距界面 "L" + 3 位数码管距离, 未启动测量时固定显示 AAA.
//   4.4 按键: S4 循环切换三个界面, S5 选择上限/下限, S6 加 0.5V, S7 减 0.5V,
//             参数在退出参数界面时生效, 参数按 0.5V 步进且 0.5V 与 5.0V 循环.
//   4.5 超声波: 电压下限 < VRB2 < 电压上限 时启动连续测距, 否则停止.
//   4.6 LED: L1/L2/L3 表示电压/测距/参数界面, L8 在测距期间按 0.1s 闪烁.
//   4.7 DAC: 测距启动时输出电压与距离按图 8 的直线关系 (20cm -> 1.0V, 80cm -> 5.0V),
//            未启动时输出 0V.
//   4.8 初始状态: 电压界面, 上限参数 4.5V, 下限参数 0.5V.

key_mode(BTN);

// 超声波测量与 ADC 刷新的等待量: 题目只给出测距精度与指示灯响应要求,
// 这里按一次测量周期取 1.2s, 让距离与电压读数稳定后再断言.
fn wait_measure() {
    run_ms(1200);
}

// 等待一个按键响应窗口 (题目要求 <= 0.2s).
fn wait_key_response() {
    run_ms(200);
}

fn press(key) {
    tap_key(key, 60);
    wait_key_response();
}

// 上电并等待一次测量: vrb2 为电位器 RB2 的电压, distance 为超声波距离.
fn boot(vrb2, distance) {
    reset(POWER_RESET);
    key_mode(BTN);
    set_voltage(AIN3, vrb2);
    set_distance_cm(distance);
    wait_measure();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// DAC 输出毫伏: PCF8591 为 8 位输出, 满量程按 5V 换算.
fn dac_mv() {
    da_value() * 5000 / 255
}

// 以 step_ms 为步长采样 L8 一段时间, 返回电平序列.
fn sample_l8(window_ms, step_ms) {
    let samples = "";
    let elapsed = 0;
    while elapsed < window_ms {
        run_ms(step_ms);
        elapsed += step_ms;
        samples += if led_on(L8) { "1" } else { "0" };
    }
    samples
}

// 统计电平序列里的跳变次数, 用于判断 L8 是否在按 0.1s 闪烁.
fn count_edges(samples) {
    let edges = 0;
    for index in 1..samples.len() {
        if samples[index - 1] != samples[index] {
            edges += 1;
        }
    }
    edges
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态 (4.6, 4.7, 4.8)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态", "电压界面, 指示灯只有 L1 亮, 未启动测量时 DA 输出 0V", || {
    // 电位器电压低于下限参数 0.5V, 测距不应启动.
    boot(0.30, 45);
    let text = display_text(30);
    let leds = led_text();
    let mv = dac_mv();

    assert_regex(text, "^U\\s+0\\.\\d\\d$", "初始状态: 应处于电压界面并显示两位小数电压");
    assert_eq(leds[0], '1', "初始状态: 电压界面下 L1 应点亮");
    assert_eq(leds[1], '0', "初始状态: L2 应熄灭");
    assert_eq(leds[2], '0', "初始状态: L3 应熄灭");
    assert_eq(leds[7], '0', "初始状态: 未启动测距时 L8 应熄灭");
    assert_eq(leds[3..7], "0000", "初始状态: L4-L7 应保持熄灭");
    assert_eq(mv, 0, "初始状态: 未启动测距时 DA 应输出 0V");
    text
});

// ---------------------------------------------------------------------------
// 检查点 2: 电压界面显示 (4.3-1)
// ---------------------------------------------------------------------------
ckpt(2, "电压界面显示", "3 位数码管显示两位小数电压", || {
    boot(2.45, 45);
    let text_245 = display_text(30);
    let value_245 = display_number(5, 8);

    set_voltage(AIN3, 4.20);
    wait_measure();
    let text_420 = display_text(30);
    let value_420 = display_number(5, 8);

    assert_regex(text_245, "^U\\s+\\d\\.\\d\\d$", "电压界面: 应显示两位小数电压");
    assert_in((value_245 * 100.0).round().to_int(), 242..=248, "电压界面: 2.45V 读数超出容差");
    assert_regex(text_420, "^U\\s+\\d\\.\\d\\d$", "电压界面: 应显示两位小数电压");
    assert_in((value_420 * 100.0).round().to_int(), 416..=424, "电压界面: 4.20V 读数超出容差");
    text_245 + " -> " + text_420
});

// ---------------------------------------------------------------------------
// 检查点 3: S4 界面切换与界面指示灯 (4.3, 4.4-1, 4.6-1)
// ---------------------------------------------------------------------------
ckpt(3, "S4 界面切换与界面指示灯", "电压 -> 测距 -> 参数 -> 电压, 界面指示灯同步", || {
    boot(2.45, 45);

    let voltage_text = display_text(30);
    let voltage_leds = led_text();

    // 每次按键后只等题目允许的 0.2s 响应窗口.
    tap_key(S4, 60);
    wait_key_response();
    let ranging_text = display_text(0);
    let ranging_leds = led_text();

    tap_key(S4, 60);
    wait_key_response();
    let param_text = display_text(0);
    let param_leds = led_text();

    tap_key(S4, 60);
    wait_key_response();
    let back_text = display_text(0);
    let back_leds = led_text();

    assert_eq(voltage_text[0], 'U', "界面切换: 上电应处于电压界面");
    assert_eq(ranging_text[0], 'L', "界面切换: 一次 S4 应切到测距界面");
    assert_eq(param_text[0], 'P', "界面切换: 两次 S4 应切到参数界面");
    assert_eq(back_text[0], 'U', "界面切换: 三次 S4 应回到电压界面");

    assert_eq(voltage_leds[0], '1', "界面指示灯: 电压界面 L1 应点亮");
    assert_eq(ranging_leds[1], '1', "界面指示灯: 测距界面 L2 应点亮");
    assert_eq(param_leds[2], '1', "界面指示灯: 参数界面 L3 应点亮");
    assert_eq(voltage_leds[1..3], "00", "界面指示灯: 电压界面下 L2/L3 应熄灭");
    assert_eq(ranging_leds[0], '0', "界面指示灯: 测距界面下 L1 应熄灭");
    assert_eq(param_leds[0], '0', "界面指示灯: 参数界面下 L1 应熄灭");
    assert_eq(back_leds, voltage_leds, "界面指示灯: 回到电压界面后应与初始一致");
    back_text
});

// ---------------------------------------------------------------------------
// 检查点 4: 测距界面与启动条件 (4.2-1, 4.3-3, 4.5)
// ---------------------------------------------------------------------------
ckpt(4, "测距界面与启动条件", "电压在上下限之间才测距, 精度 ±3cm, 否则显示 AAA", || {
    boot(2.45, 45);
    press(S4);
    let running_text = display_text(30);
    let running_distance = display_number(4, 8);

    set_distance_cm(120);
    wait_measure();
    let far_text = display_text(30);
    let far_distance = display_number(4, 8);

    // 电位器电压高于上限参数 4.5V: 测距停止.
    set_voltage(AIN3, 4.90);
    wait_measure();
    let over_limit_text = display_text(30);
    let over_limit_leds = led_text();

    // 电位器电压回到区间内: 测距重新启动.
    set_voltage(AIN3, 4.40);
    wait_measure();
    let resumed_text = display_text(30);

    assert_regex(running_text, "^L\\s+\\d+$", "测距界面: 启动测量后应显示距离数值");
    assert_in(running_distance, 42..=48, "测距精度: 45cm 的测距结果超出 ±3cm");
    assert_in(far_distance, 117..=123, "测距精度: 120cm 的测距结果超出 ±3cm");
    assert_regex(over_limit_text, "^L\\s+AAA$", "测距停止: 电压超上限时测距界面应固定显示 AAA");
    assert_eq(over_limit_leds[7], '0', "测距停止: L8 应熄灭");
    assert_regex(resumed_text, "^L\\s+\\d+$", "测距恢复: 电压回到区间内应重新显示距离");
    running_text + " -> " + over_limit_text + " -> " + resumed_text
});

// ---------------------------------------------------------------------------
// 检查点 5: 参数界面与 S5/S6/S7 (4.3-2, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(5, "参数界面与加减循环", "S6 加 0.5V, S7 减 0.5V, S5 选择上下限, 5.0 与 0.5 循环", || {
    boot(2.45, 45);
    // 从测距界面进入参数界面, 应默认选择上限参数.
    press(S4);
    press(S4);
    let initial = display_text(30);

    press(S6);
    let upper_up = display_text(30);
    press(S6);
    let upper_wrap = display_text(30);

    // 切到下限参数: 上限此时为 0.5V, 下限加 0.5V 后减回 0.5V, 再减一次回绕到 5.0V.
    press(S5);
    press(S6);
    let lower_up = display_text(30);
    press(S7);
    let lower_back = display_text(30);
    press(S7);
    let lower_wrap = display_text(30);

    assert_regex(initial, "^P  4\\.5 0\\.5$", "参数界面: 上电应为上限 4.5V 与下限 0.5V");
    assert_regex(upper_up, "^P  5\\.0 0\\.5$", "参数界面: S6 一次上限应为 5.0V");
    assert_regex(upper_wrap, "^P  0\\.5 0\\.5$", "参数界面: 上限加到 5.0V 后应循环回 0.5V");
    assert_regex(lower_up, "^P  0\\.5 1\\.0$", "参数界面: S5 选择下限后 S6 应把下限改为 1.0V");
    assert_regex(lower_back, "^P  0\\.5 0\\.5$", "参数界面: S7 应把下限减回 0.5V");
    assert_regex(lower_wrap, "^P  0\\.5 5\\.0$", "参数界面: 下限减到 0.5V 后应循环回 5.0V");
    initial + " -> " + upper_wrap + " -> " + lower_wrap
});

// ---------------------------------------------------------------------------
// 检查点 6: 参数在退出参数界面时生效 (4.4-1 说明 1)
// ---------------------------------------------------------------------------
ckpt(6, "参数退出参数界面时生效", "参数调整过程中不生效, 退出后立即生效", || {
    boot(2.45, 45);
    press(S4);
    press(S4);
    press(S5);
    // 把下限参数抬到 3.0V, 使其高于当前电位器电压 2.45V.
    press(S6);
    press(S6);
    press(S6);
    press(S6);
    press(S6);
    let param_text = display_text(30);
    let mv_inside_param = dac_mv();

    // 退出参数界面回到电压界面, 参数生效, 测距停止.
    press(S4);
    wait_measure();
    let mv_after_exit = dac_mv();

    press(S4);
    let ranging_text = display_text(30);

    assert_regex(param_text, "^P  4\\.5 3\\.0$", "参数生效: 下限参数应被抬到 3.0V");
    assert(mv_inside_param > 0, "参数生效: 参数界面内新参数不应立即生效, actual=" + mv_inside_param + "mV");
    assert_eq(mv_after_exit, 0, "参数生效: 退出参数界面后测距应停止, DA 输出 0V");
    assert_regex(ranging_text, "^L\\s+AAA$", "参数生效: 测距界面应显示 AAA");
    param_text + ", " + mv_inside_param + "mV -> " + mv_after_exit + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 7: L8 测量闪烁 (4.2-3, 4.6-2)
// ---------------------------------------------------------------------------
ckpt(7, "L8 测量闪烁", "测距期间 L8 按 0.1s 闪烁, 停止后熄灭", || {
    boot(2.45, 45);

    // 按 10ms 采样 600ms: 0.1s 间隔闪烁应至少出现 4 次跳变.
    let blinking = sample_l8(600, 10);
    let edges = count_edges(blinking);

    set_voltage(AIN3, 0.30);
    run_ms(500);
    let stopped = sample_l8(400, 10);

    assert(edges >= 4, "L8 闪烁: 0.1s 间隔闪烁在 600ms 内应出现至少 4 次跳变, actual=" + edges);
    assert_eq(stopped.contains("1"), false, "L8 闪烁: 测距停止后 L8 应保持熄灭");
    "跳变 " + edges + " 次"
});

// ---------------------------------------------------------------------------
// 检查点 8: DAC 输出与距离关系 (4.7)
// ---------------------------------------------------------------------------
ckpt(8, "DAC 输出与距离关系", "20cm 输出 1.0V, 80cm 输出 5.0V, 未启动输出 0V", || {
    boot(2.45, 20);
    let mv_20 = dac_mv();

    set_distance_cm(50);
    wait_measure();
    let mv_50 = dac_mv();

    set_distance_cm(80);
    wait_measure();
    let mv_80 = dac_mv();

    set_voltage(AIN3, 0.30);
    wait_measure();
    let mv_stopped = dac_mv();

    assert_in(mv_20, 900..=1100, "DAC 输出: 20cm 应输出 1.0V");
    assert_in(mv_80, 4900..=5000, "DAC 输出: 80cm 应输出 5.0V");
    assert(mv_50 >= mv_20 && mv_50 <= mv_80, "DAC 输出: 距离增大时输出应单调不减, 50cm=" + mv_50 + "mV");
    assert_eq(mv_stopped, 0, "DAC 输出: 测距未启动时应输出 0V");
    mv_20.to_string() + "mV, " + mv_50.to_string() + "mV, " + mv_80.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 9: 响应时间要求
// ---------------------------------------------------------------------------
ckpt(9, "响应时间要求", "按键动作响应 <= 0.2s, 指示灯动作响应 <= 0.1s", || {
    // 电压参数设为 2.0-4.0V, 电位器给 3.0V, 保证测距处于启动状态.
    boot(3.0, 30);

    // 按键响应: 按 S4 从电压界面切到测距界面, 应在 0.2 秒内完成.
    tap_key(S4, 40);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..1] != "L" {
        run_ms(10);
        key_ms += 10;
    }
    let key_page = display_text(30);

    // 指示灯响应: 切到测距界面后, 界面指示灯 L2 应在 0.1 秒内点亮.
    let led_ms = 0;
    while led_ms < 1000 && !led_on(L2) {
        run_ms(10);
        led_ms += 10;
    }
    let leds = led_text();

    assert_in(key_ms, 0..=200, "响应时间: 按键响应应在 0.2 秒内, 实际 " + key_ms.to_string() + "ms, 页面 " + key_page);
    assert_in(led_ms, 0..=100, "响应时间: 指示灯动作应在 0.1 秒内, 实际 " + led_ms.to_string() + "ms, 指示灯 " + leds);
    key_ms.to_string() + "ms, " + led_ms.to_string() + "ms"
});
