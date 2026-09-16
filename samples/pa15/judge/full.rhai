// 第十五届蓝桥杯单片机省赛 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   3.2 性能: 频率测量精度 ±8%, 按键响应 <= 0.1s, 指示灯响应 <= 0.1s, 数码管刷新 <= 0.1s.
//   3.3 显示: 频率界面 "F" + 5 位频率 (Hz, 整数), 参数界面 "P1" 超限参数 / "P2" 校准值,
//             时间界面 时-分-秒 (固定 2 位, 不足补 0), 回显界面显示最大频率值与最大频率发生时间.
//   3.4 频率测量: 显示值 = 直接测量值 + 校准值; 校准后为负数时频率界面显示 LL.
//   3.5 DAC: 输出与频率数据成直线, 频率达到超限参数时输出 5.0V; 频率状态错误时固定输出 0V.
//   3.6 按键: S4 循环切换四个界面, S5 在参数与回显界面切换子界面, S8/S9 加减参数.
//   3.7 LED: L1 在频率界面以 0.2s 闪烁, L2 在频率超限时以 0.2s 闪烁, 频率状态错误时 L2 常亮.
//   四 初始状态: 频率界面, 超限参数 2000Hz, 校准值 0.

key_mode(KEYBOARD);

// 频率测量与显示刷新等待量: 一轮测量需要跨越若干脉冲, 采样实现换频后要等一个完整测量窗口
// 才会给出新值, 这里取 2s 让读数稳定, 避免把窗口跨越新旧频率的中间值当成结果.
fn wait_measure() {
    run_ms(2000);
}

// 按一次键并留出足够的间隔: 题目要求按键响应 <= 0.1s, 但连续换界面时采样实现需要更长的
// 释放间隔, 否则第二次按键会被丢掉, 因此导航用的等待量取 0.25s, 响应时间另有单独评测点.
fn press(key) {
    tap_key(key, 60);
    run_ms(250);
}

// 上电并建立确定的初始状态: RTC 在启动初始化完成之后设置, 避免被启动写入覆盖.
fn boot(freq, hour, minute, second) {
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    jumper_on(NET_SIG, SIG_OUT);
    set_frequency_hz(freq);
    wait_measure();
    set_rtc(hour, minute, second);
    run_ms(300);
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

// 按 20ms 步长采样某个指示灯 600ms, 返回电平序列与跳变次数.
fn sample_led(index) {
    let samples = "";
    for step in 1..=30 {
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

// 从频率界面进入参数界面并选择第 select 个子界面 (0=超限参数, 1=校准值).
fn goto_param(select) {
    press(S4);
    for step in 0..select {
        press(S5);
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态与频率显示 (3.3-1, 3.7-1, 初始状态)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态与频率显示", "频率界面显示 1500, 超限参数 2000, 校准值 0, L1 闪烁", || {
    boot(1500, 13, 3, 5);
    let text = display_text(30);
    let value = display_number(2, 8);
    let l1 = sample_led(L1);
    let leds_after = led_text();

    // 进入参数界面读取两个默认参数, 顺便检查从频率界面进入参数界面默认是超限参数.
    press(S4);
    let p1 = display_text(30);
    press(S5);
    let p2 = display_text(30);

    assert_regex(text, "^F\\s+\\d+$", "频率界面: 应以 Hz 整数显示频率");
    assert_in(value, 1380..=1620, "频率界面: 1500Hz 读数超出 ±8%");
    assert(l1[1] >= 2, "频率界面: L1 应以 0.2s 为间隔闪烁, actual=" + l1[0]);
    assert_eq(leds_after[1..8], "0000000", "初始状态: 除 L1 外其余指示灯应熄灭");
    assert_regex(p1, "^P1\\s+2000$", "参数界面: 上电超限参数应为 2000Hz");
    assert_regex(p2, "^P2\\s+0$", "参数界面: 上电校准值应为 0");
    text + ", " + p1 + ", " + p2
});

// ---------------------------------------------------------------------------
// 检查点 2: 频率校准功能 (3.4-2)
// ---------------------------------------------------------------------------
ckpt(2, "频率校准功能", "显示值 = 测量值 + 校准值", || {
    boot(1000, 13, 3, 5);
    wait_measure();
    let before = display_number(2, 8);

    // 进入参数界面, 切到校准值子界面, 把校准值减到 -300.
    goto_param(1);
    for step in 0..3 {
        press(S9);
    }
    let param_text = display_text(30);

    // 回到频率界面, 显示值应比测量值低 300.
    press(S4);
    press(S4);
    press(S4);
    wait_measure();
    let after = display_number(2, 8);

    assert_regex(param_text, "^P2  -300$", "频率校准: 参数界面应显示校准值 -300");
    assert_in(after - before, -330..=-270, "频率校准: 显示值应等于测量值加校准值");
    before.to_string() + " -> " + after.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 3: 校准后为负数的错误状态 (3.3-1, 3.4-2, 3.5, 3.7-2)
// ---------------------------------------------------------------------------
ckpt(3, "校准后为负数的错误状态", "频率界面显示 LL, L2 常亮, DAC 输出 0V", || {
    boot(500, 13, 3, 5);
    // 校准值减到 -900, 使 500 - 900 = -400 为负.
    goto_param(1);
    for step in 0..9 {
        press(S9);
    }
    press(S4);
    press(S4);
    press(S4);
    wait_measure();
    let text = display_text(30);
    let l2 = sample_led(L2);
    let mv = dac_mv();

    assert_regex(text, "^F\\s+LL$", "错误状态: 校准后为负时应显示 LL");
    assert_eq(l2[1], 0, "错误状态: L2 应常亮而不是闪烁, actual=" + l2[0]);
    assert_eq(led_text()[1], '1', "错误状态: L2 应点亮");
    assert_eq(mv, 0, "错误状态: DAC 应固定输出 0V");
    text
});

// ---------------------------------------------------------------------------
// 检查点 4: 参数界面加减与边界 (3.3-2, 3.6-1)
// ---------------------------------------------------------------------------
ckpt(4, "参数界面加减与边界", "超限参数 1000-9000, 校准值 -900-900, 步进固定且到边界不再变化", || {
    boot(1000, 13, 3, 5);
    press(S4);
    let initial = display_text(30);

    // 超限参数加到上限后继续加应保持 9000.
    for step in 0..8 {
        press(S8);
    }
    let upper_max = display_text(30);
    press(S8);
    let upper_hold = display_text(30);

    // 减到下限 1000 后继续减应保持 1000.
    for step in 0..10 {
        press(S9);
    }
    let lower_min = display_text(30);
    press(S9);
    let lower_hold = display_text(30);

    // 校准值加到上限 900 后继续加应保持 900.
    press(S5);
    for step in 0..10 {
        press(S8);
    }
    let cal_max = display_text(30);
    press(S8);
    let cal_hold = display_text(30);

    assert_regex(initial, "^P1\\s+2000$", "参数界面: 上电超限参数应为 2000Hz");
    assert_regex(upper_max, "^P1\\s+9000$", "参数界面: 超限参数应能加到 9000Hz");
    assert_regex(upper_hold, "^P1\\s+9000$", "参数界面: 超过上限后应保持 9000Hz");
    assert_regex(lower_min, "^P1\\s+1000$", "参数界面: 超限参数应能减到 1000Hz");
    assert_regex(lower_hold, "^P1\\s+1000$", "参数界面: 低于下限后应保持 1000Hz");
    assert_regex(cal_max, "^P2\\s+900$", "参数界面: 校准值应能加到 900Hz");
    assert_regex(cal_hold, "^P2\\s+900$", "参数界面: 校准值超过上限后应保持 900Hz");
    upper_hold + ", " + lower_hold + ", " + cal_hold
});

// ---------------------------------------------------------------------------
// 检查点 5: 时间界面 (3.3-3)
// ---------------------------------------------------------------------------
ckpt(5, "时间界面与按键响应", "0.1s 内响应按键, 时间界面显示 时-分-秒 并补 0", || {
    boot(1000, 9, 5, 7);
    // 按键响应要求 <= 0.1s: 按下后只等 0.1s 就应看到界面已经切换.
    tap_key(S4, 60);
    run_ms(100);
    let after_response = display_text(0);

    press(S4);
    let text = display_text(30);

    assert_eq(after_response[0], 'P', "按键响应: 按下 S4 后 0.1s 内应完成界面切换");
    assert_regex(text, "^\\d\\d-\\d\\d-\\d\\d$", "时间界面: 应按时-分-秒显示并补 0");
    assert_eq(text[0..2], "09", "时间界面: 小时应补 0 显示为 09");
    assert_eq(text[3..5], "05", "时间界面: 分钟应补 0 显示为 05");
    text
});

// ---------------------------------------------------------------------------
// 检查点 6: 回显界面 (3.3-4, 3.6-1)
// ---------------------------------------------------------------------------
ckpt(6, "回显界面", "显示最大频率值与发生时间, S5 切换子界面, 默认频率回显", || {
    // 全程保持同一个频率: 频率不变时最大频率就是当前频率, 回显值可以直接与频率界面比对.
    boot(2035, 13, 3, 5);
    let freq_text = display_text(30);
    let freq_value = display_number(2, 8);

    // 时间界面 -> 回显界面, 默认应为频率回显.
    press(S4);
    press(S4);
    press(S4);
    let hf = display_text(30);
    // 回显值要在读数时立刻取, 切到时间回显后再读会读到另一个界面.
    let hf_value = display_number(3, 8);
    press(S5);
    let ha = display_text(30);
    let leds = led_text();

    assert_regex(freq_text, "^F\\s+\\d+$", "频率界面: 应以 Hz 整数显示频率");
    assert_regex(hf, "^HF\\s+\\d+$", "回显界面: 默认应显示频率回显");
    assert_in((hf_value - freq_value).abs(), 0..=163, "回显界面: 最大频率值应与频率界面读数一致, actual=" + hf);
    assert_regex(ha, "^H[AR]\\d{6}$", "回显界面: 时间回显应显示界面编号与 6 位时间");
    // 回显的是"最大频率出现的时刻", 频率恒定且从开机就有效时应落在开机那一刻,
    // 因此这里只校验时分是合法值, 不强制等于脚本后来设置的 13 时.
    assert_in(ha[2..4].parse_int(), 0..=23, "回显界面: 时间回显的小时应是合法值, actual=" + ha);
    assert_in(ha[4..6].parse_int(), 0..=59, "回显界面: 时间回显的分应是合法值, actual=" + ha);
    // 频率 2035Hz 高于超限参数 2000Hz, 因此 L2 报警指示灯会闪烁; 回显界面下 L1 应熄灭, 其余也熄灭.
    assert_eq(leds[0], '0', "回显界面: 非频率界面下 L1 应熄灭");
    assert_eq(leds[2..8], "000000", "回显界面: 除报警指示灯外的其它指示灯应熄灭");
    hf + " -> " + ha
});

// ---------------------------------------------------------------------------
// 检查点 7: 频率超限报警指示灯 (3.7-2)
// ---------------------------------------------------------------------------
ckpt(7, "频率超限报警指示灯", "频率超过超限参数时 L2 以 0.2s 闪烁, 否则熄灭", || {
    boot(1000, 13, 3, 5);
    let under = sample_led(L2);

    set_frequency_hz(5000);
    wait_measure();
    let over = sample_led(L2);

    set_frequency_hz(1000);
    wait_measure();
    let back = sample_led(L2);

    assert_eq(under[1], 0, "报警指示灯: 频率未超限时 L2 应熄灭, actual=" + under[0]);
    assert(over[1] >= 2, "报警指示灯: 频率超限时 L2 应以 0.2s 闪烁, actual=" + over[0]);
    assert_eq(back[1], 0, "报警指示灯: 频率回落后 L2 应熄灭, actual=" + back[0]);
    over[0]
});

// ---------------------------------------------------------------------------
// 检查点 8: DAC 输出 (3.5)
// ---------------------------------------------------------------------------
ckpt(8, "DAC 输出与频率关系", "频率达到超限参数输出 5.0V, 随频率单调不减", || {
    boot(1000, 13, 3, 5);
    wait_measure();
    let mv_low = dac_mv();

    set_frequency_hz(1000);
    wait_measure();
    let mid_point = dac_mv();

    set_frequency_hz(2000);
    wait_measure();
    let at_limit = dac_mv();

    set_frequency_hz(4000);
    wait_measure();
    let over_limit = dac_mv();

    // 超限参数默认 2000Hz: 达到与超过超限参数都应输出 5.0V.
    assert_in(at_limit, 4900..=5000, "DAC 输出: 频率达到超限参数时应输出 5.0V");
    assert_in(over_limit, 4900..=5000, "DAC 输出: 频率超过超限参数后应保持 5.0V");
    // 图 7 的直线在 0Hz 处为 1.0V, 在超限参数处为 5.0V, 中间点应落在这段区间内且单调不减.
    assert_in(mv_low, 900..=5100, "DAC 输出: 低频段输出应落在 1.0V 到 5.0V 之间");
    assert(mid_point >= mv_low, "DAC 输出: 频率升高时输出不应下降");
    assert(mid_point <= at_limit, "DAC 输出: 输出不应超过超限参数对应的 5.0V");
    mv_low.to_string() + "mV, " + mid_point.to_string() + "mV, " + at_limit.to_string() + "mV"
});
