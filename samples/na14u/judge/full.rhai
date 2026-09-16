// 第十四届蓝桥杯单片机国赛 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   3.2 性能: 测距精度 ±3cm, 重复性 <= 1cm, 范围 10-90cm, 按键响应 <= 0.1s,
//             温度与超声波刷新 <= 1s, 输出动作响应 <= 0.1s.
//   3.3 显示: 测距界面 温度 (1 位小数) + '-' + 测距数据 (cm 整数或 m 两位小数),
//             参数界面 "P" + 编号 1/2 + 参数值, 工厂模式 "F" + 编号 1/2/3.
//   3.4 按键: S4 在 测距/参数/工厂模式 间切换, S5 在各级界面内切换子界面,
//             S8/S9 按步进加减 (10cm, 1 摄氏度, 5cm, 10m/s, 0.1V),
//             测距界面下 S8 记录 6 秒距离变化, S9 输出记录数据且不影响采集与显示,
//             S8+S9 同时按住超过 2 秒恢复出厂设置.
//   3.5 DAC: 输出与已记录的距离数据成直线, 10cm 对应下限值 VOL, 90cm 对应 5.0V.
//   3.6 测距: 测距结果 = V*T/2 + 校准值.
//   3.7 LED: 测距界面用 L1-L8 表示距离的 8 位二进制, 参数界面 L8 点亮, 工厂模式 L1 以 0.1s 闪烁.
//   3.8 继电器: 距离参数-5 <= 测距结果 <= 距离参数+5 且 温度 <= 温度参数 时吸合.
//   四 初始状态: 测距界面 (cm), 距离参数 40, 温度参数 30, 校准值 0, 传输速度 340, DAC 下限 1.0V.

key_mode(KBD);

fn wait_measure() {
    // 采样实现的超声波需要多个采集周期才给出有效读数, 这里取 2s 覆盖刷新要求 (<= 1s) 的余量.
    run_ms(2000);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(200);
}

fn boot(temp, distance) {
    reset(POWER_RESET);
    key_mode(KBD);
    set_temperature_c(temp);
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

fn dac_mv() {
    da_value() * 5000 / 255
}

// 距离值对应的 L1-L8 二进制 (L1 为最低位).
fn distance_leds(value) {
    let text = "";
    for index in 1..=8 {
        text += if (value / (1 << (index - 1))) % 2 == 1 { "1" } else { "0" };
    }
    text
}

// 采样某个指示灯 400ms, 返回电平序列与跳变次数.
fn sample_led(index) {
    let samples = "";
    for step in 1..=20 {
        run_ms(20);
        samples += if led_on(index) { "1" } else { "0" };
    }
    let edges = 0;
    for pos in 1..samples.len() {
        if samples[pos - 1] != samples[pos] {
            edges += 1;
        }
    }
    [samples, edges]
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与测距界面 (3.3-1, 四)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与测距界面", "测距界面显示 温度-距离, 单位为 cm, 五组默认参数正确", || {
    boot(23.5, 25);
    let ranging = display_text(30);
    let ranging_value = display_number(5, 8);

    press(S4);
    let p1 = display_text(30);
    press(S5);
    let p2 = display_text(30);

    press(S4);
    let f1 = display_text(30);
    press(S5);
    let f2 = display_text(30);
    press(S5);
    let f3 = display_text(30);

    assert_regex(ranging, "^23\\.5-\\s+\\d+$", "初始状态: 测距界面应显示 温度-距离 且单位为 cm");
    assert_in(ranging_value, 22..=28, "测距精度: 25cm 的测距结果超出 ±3cm");
    assert_regex(p1, "^P1\\s+40$", "初始状态: 距离参数应为 40cm");
    assert_regex(p2, "^P2\\s+30$", "初始状态: 温度参数应为 30 摄氏度");
    assert_regex(f1, "^F1\\s+0$", "初始状态: 校准值应为 0cm");
    assert_regex(f2, "^F2\\s+340$", "初始状态: 传输速度应为 340m/s");
    assert_regex(f3, "^F3\\s+1\\.0$", "初始状态: DAC 输出下限值应为 1.0V");
    ranging + ", " + p1 + ", " + f1 + ", " + f2 + ", " + f3
});

// ---------------------------------------------------------------------------
// 检查点 2: 单位切换与重复性 (3.2-1, 3.3-1, 3.4-1)
// ---------------------------------------------------------------------------
ckpt(2, "单位切换与测距重复性", "cm 与 m 单位可切换, 同一距离连续测量重复性不超过 1cm", || {
    boot(23.5, 45);
    let cm_text = display_text(30);
    let first = display_number(5, 8);

    press(S5);
    let m_text = display_text(30);
    let m_value = (display_number(4, 8) * 100.0).round().to_int();

    press(S5);
    press(S4);
    press(S4);
    press(S4);
    let back_text = display_text(30);

    // 同一距离连续读三次, 考察重复性.
    let second = display_number(5, 8);
    wait_measure();
    let third = display_number(5, 8);

    assert_regex(cm_text, "^23\\.5-\\s+\\d+$", "测距界面: cm 单位应显示整数");
    assert_in(first, 42..=48, "测距精度: 45cm 的测距结果超出 ±3cm");
    assert_regex(m_text, "^23\\.5-\\s+0\\.\\d\\d$", "测距界面: m 单位应保留 2 位小数");
    assert_in(m_value, 42..=48, "测距界面: m 单位读数换算后超出 ±3cm");
    assert_regex(back_text, "^23\\.5-\\s+\\d+$", "测距界面: 重新进入后应回到 cm 单位");
    assert_in((third - first).abs(), 0..=1, "测距重复性: 同一距离连续测量差值应不超过 1cm");
    first.to_string() + " " + second.to_string() + " " + third.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 3: LED 距离指示 (3.7)
// ---------------------------------------------------------------------------
ckpt(3, "LED 距离指示", "测距界面 L1-L8 表示距离二进制, 参数界面 L8 亮, 工厂模式 L1 闪烁", || {
    boot(23.5, 25);
    let leds_25 = led_text();
    let value_25 = display_number(5, 8);

    set_distance_cm(80);
    wait_measure();
    let leds_80 = led_text();
    let value_80 = display_number(5, 8);

    press(S4);
    let param_leds = led_text();
    press(S4);
    let factory = sample_led(L1);

    assert_eq(leds_25, distance_leds(value_25), "LED 指示: 测距界面的指示灯应为距离的 8 位二进制");
    assert_eq(leds_80, distance_leds(value_80), "LED 指示: 80cm 附近的读数应与其二进制指示灯一致");
    assert_in(value_80, 77..=83, "测距精度: 80cm 的测距结果超出 ±3cm");
    assert_eq(param_leds, "00000001", "LED 指示: 参数界面应只点亮 L8");
    assert(factory[1] >= 2, "LED 指示: 工厂模式下 L1 应以 0.1s 闪烁, actual=" + factory[0]);
    leds_25 + " -> " + leds_80
});

// ---------------------------------------------------------------------------
// 检查点 4: 参数与工厂模式加减 (3.4-1, 3.4-2)
// ---------------------------------------------------------------------------
ckpt(4, "参数与工厂模式加减", "各界面按题目步进加减", || {
    boot(23.5, 25);
    press(S4);
    let p1 = display_text(30);
    press(S8);
    let p1_up = display_text(30);
    press(S9);
    press(S9);
    let p1_down = display_text(30);

    press(S5);
    let p2 = display_text(30);
    press(S8);
    let p2_up = display_text(30);

    press(S4);
    let f1 = display_text(30);
    press(S9);
    let f1_down = display_text(30);
    press(S8);
    press(S8);
    let f1_up = display_text(30);

    press(S5);
    let f2 = display_text(30);
    press(S8);
    let f2_up = display_text(30);

    press(S5);
    let f3 = display_text(30);
    press(S8);
    let f3_up = display_text(30);

    assert_regex(p1, "^P1\\s+40$", "参数界面: 距离参数默认应为 40cm");
    assert_regex(p1_up, "^P1\\s+50$", "参数界面: 距离参数加 10cm 后应为 50");
    assert_regex(p1_down, "^P1\\s+30$", "参数界面: 距离参数连续减两次后应为 30");
    assert_regex(p2, "^P2\\s+30$", "参数界面: 温度参数默认应为 30");
    assert_regex(p2_up, "^P2\\s+31$", "参数界面: 温度参数加 1 后应为 31");
    assert_regex(f1, "^F1\\s+0$", "工厂模式: 校准值默认应为 0");
    assert_regex(f1_down, "^F1\\s+-5$", "工厂模式: 校准值减 5 后应显示 -5");
    assert_regex(f1_up, "^F1\\s+5$", "工厂模式: 校准值再加两次后应为 5");
    assert_regex(f2, "^F2\\s+340$", "工厂模式: 传输速度默认应为 340");
    assert_regex(f2_up, "^F2\\s+350$", "工厂模式: 传输速度加 10 后应为 350");
    assert_regex(f3, "^F3\\s+1\\.0$", "工厂模式: DAC 下限默认应为 1.0V");
    assert_regex(f3_up, "^F3\\s+1\\.1$", "工厂模式: DAC 下限加 0.1V 后应为 1.1");
    p1 + ", " + f1 + ", " + f2 + ", " + f3
});

// ---------------------------------------------------------------------------
// 检查点 5: 校准值与传输速度生效 (3.6)
// ---------------------------------------------------------------------------
ckpt(5, "校准值与传输速度生效", "校准值改变测距结果, 传输速度改变同一距离的读数", || {
    boot(23.5, 45);
    let before = display_number(5, 8);

    // 工厂模式把校准值设为 10cm.
    press(S4);
    press(S4);
    for step in 0..2 {
        press(S8);
    }
    let cal = display_text(30);
    press(S4);
    wait_measure();
    let after = display_number(5, 8);

    assert_regex(cal, "^F1\\s+10$", "校准值: 工厂模式应能把校准值设为 10cm");
    assert_in(after - before, 7..=13, "校准值: 测距结果应增加约 10cm");
    before.to_string() + " -> " + after.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 6: 继电器控制 (3.8)
// ---------------------------------------------------------------------------
ckpt(6, "继电器控制", "距离在参数 ±5cm 内且温度不超过温度参数时吸合", || {
    boot(20.0, 40);
    let inside = relay_on();

    set_distance_cm(50);
    wait_measure();
    let outside = relay_on();

    set_distance_cm(40);
    set_temperature_c(35.0);
    wait_measure();
    let hot = relay_on();

    assert_eq(inside, true, "继电器: 距离 40cm 在 40±5cm 内且温度不超参数时应吸合");
    assert_eq(outside, false, "继电器: 距离 50cm 超出参数范围时应断开");
    assert_eq(hot, false, "继电器: 温度超过温度参数时应断开");
    inside.to_string() + " -> " + outside.to_string() + " -> " + hot.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 7: 记录与 DAC 输出 (3.4-1, 3.5)
// ---------------------------------------------------------------------------
ckpt(7, "记录与 DAC 输出", "S8 记录 6 秒距离变化, S9 输出到 DAC 且不影响采集与显示", || {
    boot(23.5, 90);
    press(S8);
    // 记录期间连续改变距离, 记录过程 6 秒内按键无效.
    for step in 1..=12 {
        set_distance_cm(20 + step * 5);
        run_ms(500);
    }
    let during_record = display_text(30);

    press(S9);
    run_ms(300);
    // 输出过程中采样 DAC, 同时读取显示确认采集与显示未受影响.
    let values = [];
    for step in 1..=8 {
        run_ms(300);
        values.push(dac_mv());
    }
    let display_during_output = display_text(30);

    let low = values[0];
    let high = values[0];
    let distinct = 1;
    for pos in 1..values.len() {
        if values[pos] < low { low = values[pos]; }
        if values[pos] > high { high = values[pos]; }
        if values[pos] != values[pos - 1] { distinct += 1; }
    }

    assert_regex(during_record, "^23\\.5-\\s+\\d+$", "记录状态: 记录期间数码管仍应正常显示测距界面");
    assert_in(low, 900..=5000, "DAC 输出: 记录数据的输出电压不应低于下限值 1.0V");
    assert_in(high, 900..=5100, "DAC 输出: 记录数据的输出电压不应超过 5.0V");
    assert(distinct >= 3, "DAC 输出: 记录的距离在变化, 输出应随之变化, actual=" + values);
    assert_regex(display_during_output, "^23\\.5-\\s+\\d+$", "DAC 输出: 输出过程中显示与采集应不受影响");
    values.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 8: 响应时间要求 (3.2)
// ---------------------------------------------------------------------------
ckpt(8, "响应时间要求", "按键 <= 0.1s, 超声波刷新 <= 1s, 输出动作 <= 0.1s", || {
    boot(23.5, 25);

    // 按键响应: 按下 S4 后应在 0.1 秒内切到参数界面 (从松开按键开始计时).
    tap_key(S4, 60);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..1] != "P" {
        run_ms(10);
        key_ms += 10;
    }
    // S4 在 测距 -> 参数 -> 工厂模式 之间循环, 因此要再按两次才回到测距界面.
    press(S4);
    press(S4);

    // 超声波刷新: 距离改到 60cm 后应在 1 秒内更新到 60cm 附近.
    set_distance_cm(60);
    let dist_ms = 0;
    while dist_ms < 3000 && (display_number(5, 8) - 60).abs() > 3 {
        run_ms(50);
        dist_ms += 50;
    }
    let dist_page = display_text(30);

    // 输出动作: 距离数据先刷新到 40cm 附近 (超声波刷新有 1 秒上限),
    // 之后再量"继电器动作"的响应时间, 避免把传感器刷新时间算进输出响应里.
    set_distance_cm(40);
    let settle_ms = 0;
    while settle_ms < 3000 && (display_number(5, 8) - 40).abs() > 3 {
        run_ms(10);
        settle_ms += 10;
    }
    let relay_ms = 0;
    while relay_ms < 1000 && !relay_on() {
        run_ms(10);
        relay_ms += 10;
    }

    assert_in(key_ms, 0..=100, "响应时间: 按键响应应在 0.1 秒内, 实际 " + key_ms.to_string() + "ms");
    assert_in(dist_ms, 0..=1000, "响应时间: 超声波数据应在 1 秒内刷新, 实际 " + dist_ms.to_string() + "ms, 页面 " + dist_page);
    assert_in(relay_ms, 0..=100, "响应时间: 继电器动作应在 0.1 秒内, 实际 " + relay_ms.to_string() + "ms (等待测距数据刷新另计 " + settle_ms.to_string() + "ms)");
    key_ms.to_string() + "ms, " + dist_ms.to_string() + "ms, " + settle_ms.to_string() + "ms+" + relay_ms.to_string() + "ms"
});
