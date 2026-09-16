// 第十三届蓝桥杯单片机国赛 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.2 性能: 测距精度 ±3cm, 频率精度 ±8%, 按键响应 <= 0.2s, 指示灯响应 <= 0.1s.
//   4.3 显示: 频率界面 "F" + 6 位频率 (Hz 或 KHz, KHz 保留 1 位小数),
//             湿度界面 "H" + 2 位湿度整数, 测距界面 "A" + 3 位距离 (CM 或 M, M 保留 2 位小数),
//             参数界面 "p1"/"p2"/"p3" + 频率/湿度/距离参数.
//   4.4 按键: S4 循环切换四个界面, S5 在参数界面切换三组参数,
//             S6/S7 按各自步进加减参数并在边界循环, 测距界面 S6 切换 CM/M, 频率界面 S7 切换 Hz/KHz.
//   4.5 湿度: 电位器 RB2 电压 (0V-5V) 线性对应湿度 0%-100%RH.
//   4.6 DAC: 输出电压与湿度数据成直线关系, 直线经过 (湿度参数, 1.0V) 与 (80%RH, 5.0V).
//   4.7 继电器: 测距结果大于距离参数时吸合, 否则断开.
//   4.8 脉冲输出: 频率数据大于频率参数时电机驱动引脚输出 1KHz 80% 占空比, 否则 1KHz 20%.
//   4.9 LED: L1/L2/L3 分别表示频率/湿度/测距界面, 对应参数界面下以 0.1s 闪烁,
//            L4/L5/L6 表示频率/湿度/距离数据超过各自参数, L7/L8 保持熄灭.
//   4.10 E2PROM: 继电器开关次数保存在地址 0; 湿度界面长按 S7 超过 1s 松开清零.
//   4.11 初始状态: 频率界面 (Hz), 距离界面单位 CM, 参数默认 9.0KHz / 40% / 0.6M.

key_mode(BTN);

// 频率测量窗口与数据刷新等待量: 一轮测量需要跨越若干脉冲, 这里取 2s 让读数稳定.
fn wait_measure() {
    run_ms(2000);
}

// 等待一个按键响应窗口 (题目要求 <= 0.2s).
fn wait_key_response() {
    run_ms(200);
}

fn press(key) {
    tap_key(key, 60);
    wait_key_response();
}

// 上电并建立确定的初始状态: EEPROM 计数从 0 开始, 频率/电位器/距离按参数给定.
fn boot(freq, vrb2, distance) {
    reset(POWER_RESET);
    key_mode(BTN);
    jumper_on(NET_SIG, SIG_OUT);
    set_eeprom(0, 0);
    set_frequency_hz(freq);
    set_voltage(AIN3, vrb2);
    set_distance_cm(distance);
    run_ms(1500);
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

// 按 20ms 步长采样某个指示灯 600ms, 返回电平序列, 用于判断 0.1s 闪烁.
fn sample_led(index) {
    let samples = "";
    for step in 1..=30 {
        run_ms(20);
        samples += if led_on(index) { "1" } else { "0" };
    }
    samples
}

fn count_edges(samples) {
    let edges = 0;
    for index in 1..samples.len() {
        if samples[index - 1] != samples[index] {
            edges += 1;
        }
    }
    edges
}

// 采样电机驱动引脚 2ms, 返回导通的采样数与跳变次数, 用于判断 1KHz 与占空比.
fn sample_motor() {
    let on = 0;
    let samples = "";
    for step in 1..=80 {
        run_us(25);
        let high = motor_on();
        if high {
            on += 1;
        }
        samples += if high { "1" } else { "0" };
    }
    [on, count_edges(samples)]
}

// 从频率界面进入参数界面 (S4 三次), 再按选择的参数组按 S5.
fn goto_param(select_times) {
    press(S4);
    press(S4);
    press(S4);
    for step in 0..select_times {
        press(S5);
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态与频率界面 (4.3-1, 4.9-1, 4.11)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态与频率界面", "频率界面显示 Hz, 参数默认 9.0KHz/40%/0.6M, L1 点亮", || {
    boot(245, 0.0, 45);
    let freq_page = display_text(30);
    let freq_leds = led_text();

    // 进入参数界面依次读取三组默认参数: 从测距界面进入参数界面应默认频率参数.
    press(S4);
    press(S4);
    let ranging_leds = led_text();
    press(S4);
    let p1 = display_text(30);
    press(S5);
    let p2 = display_text(30);
    press(S5);
    let p3 = display_text(30);

    assert_regex(freq_page, "^F\\s+245$", "初始状态: 频率界面应以 Hz 显示 245");
    assert_eq(freq_leds, "10000000", "初始状态: 频率界面下只应点亮 L1");
    assert_eq(ranging_leds[2], '1', "初始状态: 测距界面下 L3 应点亮");
    assert_regex(p1, "^P1\\s+9\\.0$", "初始状态: 频率参数应为 9.0KHz");
    assert_regex(p2, "^P2\\s+40$", "初始状态: 湿度参数应为 40");
    assert_regex(p3, "^P3\\s+0\\.6$", "初始状态: 距离参数应为 0.6M");
    p1 + ", " + p2 + ", " + p3
});

// ---------------------------------------------------------------------------
// 检查点 2: 频率界面显示与单位切换 (4.2-2, 4.3-1, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(2, "频率界面与 Hz/KHz 切换", "6 位显示, 精度 ±8%, S7 切换 Hz 与 KHz", || {
    boot(20000, 0.0, 45);
    let hz_text = display_text(30);
    let hz_value = display_number(2, 8);

    press(S7);
    let khz_text = display_text(30);
    let khz_value = display_number(2, 8);

    press(S7);
    let back_text = display_text(30);

    assert_regex(hz_text, "^F\\s+\\d+$", "频率界面: Hz 显示格式错误");
    assert_in(hz_value, 18400..=21600, "频率界面: 20000Hz 读数超出 ±8%");
    assert_regex(khz_text, "^F\\s+\\d+\\.\\d$", "频率界面: KHz 应保留 1 位小数");
    assert_in((khz_value * 10.0).round().to_int(), 184..=216, "频率界面: KHz 读数超出 ±8%");
    assert_eq(back_text[0], 'F', "频率界面: 再按 S7 应回到 Hz 显示");
    hz_text + " -> " + khz_text
});

// ---------------------------------------------------------------------------
// 检查点 3: 湿度界面 (4.3-2, 4.5, 4.9-2)
// ---------------------------------------------------------------------------
ckpt(3, "湿度界面", "RB2 电压线性对应湿度, 2 位整数显示, L2 点亮", || {
    boot(1000, 1.75, 45);
    press(S4);
    let low_text = display_text(30);
    let low_leds = led_text();
    let low_value = display_number(2, 8);

    set_voltage(AIN3, 4.60);
    wait_measure();
    let high_text = display_text(30);
    let high_value = display_number(2, 8);

    assert_regex(low_text, "^H\\s+\\d+$", "湿度界面: 应显示整数湿度");
    assert_in(low_value, 33..=37, "湿度界面: 1.75V 应约对应 35%RH");
    assert_in(high_value, 90..=94, "湿度界面: 4.60V 应约对应 92%RH");
    assert_eq(low_leds[1], '1', "湿度界面: L2 应点亮");
    assert_eq(low_leds[0], '0', "湿度界面: L1 应熄灭");
    low_text + " -> " + high_text
});

// ---------------------------------------------------------------------------
// 检查点 4: 测距界面与单位切换 (4.2-1, 4.3-3, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(4, "测距界面与 CM/M 切换", "距离精度 ±3cm, 默认 CM, S6 切换 CM 与 M", || {
    boot(1000, 0.0, 45);
    press(S4);
    press(S4);
    let cm_text = display_text(30);
    let cm_value = display_number(2, 8);
    let cm_leds = led_text();

    press(S6);
    let m_text = display_text(30);
    let m_value = display_number(2, 8);

    press(S6);
    let cm_back = display_text(30);

    assert_regex(cm_text, "^A\\s+\\d+$", "测距界面: 默认应以 CM 显示");
    assert_in(cm_value, 42..=48, "测距界面: 45cm 的测距结果超出 ±3cm");
    assert_in((m_value * 100.0).round().to_int(), 42..=48, "测距界面: M 单位读数超出 ±3cm");
    assert_regex(m_text, "^A\\s+0\\.\\d\\d$", "测距界面: M 单位应保留 2 位小数");
    assert_regex(cm_back, "^A\\s+\\d+$", "测距界面: 再按 S6 应回到 CM 显示");
    assert_eq(cm_leds[2], '1', "测距界面: L3 应点亮");
    cm_text + " -> " + m_text
});

// ---------------------------------------------------------------------------
// 检查点 5: 参数界面切换, 加减与边界循环 (4.3-4, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(5, "参数加减与边界循环", "S5 切换三组参数, S6/S7 按步进加减并在边界循环", || {
    boot(1000, 0.0, 45);
    goto_param(0);
    let p1 = display_text(30);

    // 频率参数 9.0 -> 12.0 (6 步), 再加一次回绕到 1.0, 再减一次回到 12.0.
    for step in 0..6 {
        press(S6);
    }
    let p1_max = display_text(30);
    press(S6);
    let p1_wrap = display_text(30);
    press(S7);
    let p1_back = display_text(30);

    // 湿度参数 40 -> 60 (2 步), 再加一次回绕到 10.
    press(S5);
    press(S6);
    press(S6);
    let p2_max = display_text(30);
    press(S6);
    let p2_wrap = display_text(30);
    press(S7);
    let p2_back = display_text(30);

    // 距离参数 0.6 -> 1.2 (6 步), 再加一次回绕到 0.1.
    press(S5);
    for step in 0..6 {
        press(S6);
    }
    let p3_max = display_text(30);
    press(S6);
    let p3_wrap = display_text(30);

    assert_regex(p1, "^P1\\s+9\\.0$", "参数界面: 上电频率参数应为 9.0KHz");
    assert_regex(p1_max, "^P1\\s+12\\.0$", "参数界面: 频率参数加到 12.0KHz 应停止");
    assert_regex(p1_wrap, "^P1\\s+1\\.0$", "参数界面: 频率参数超过上限应回绕到 1.0KHz");
    assert_regex(p1_back, "^P1\\s+12\\.0$", "参数界面: 频率参数低于下限应回绕到 12.0KHz");
    assert_regex(p2_max, "^P2\\s+60$", "参数界面: 湿度参数加到 60 应停止");
    assert_regex(p2_wrap, "^P2\\s+10$", "参数界面: 湿度参数超过上限应回绕到 10");
    assert_regex(p2_back, "^P2\\s+60$", "参数界面: 湿度参数低于下限应回绕到 60");
    assert_regex(p3_max, "^P3\\s+1\\.2$", "参数界面: 距离参数加到 1.2M 应停止");
    assert_regex(p3_wrap, "^P3\\s+0\\.1$", "参数界面: 距离参数超过上限应回绕到 0.1M");
    p1_wrap + ", " + p2_wrap + ", " + p3_wrap
});

// ---------------------------------------------------------------------------
// 检查点 6: 界面指示灯与参数界面闪烁 (4.2-4, 4.9-1/2/3)
// ---------------------------------------------------------------------------
ckpt(6, "界面指示灯与参数界面闪烁", "L1/L2/L3 指示对应界面, 参数界面下按 0.1s 闪烁", || {
    boot(1000, 0.0, 45);
    let freq_leds = led_text();

    press(S4);
    let humidity_leds = led_text();

    press(S4);
    let ranging_leds = led_text();

    // 参数界面下: 频率参数界面闪烁 L1, 湿度参数界面闪烁 L2, 距离参数界面闪烁 L3.
    press(S4);
    let p1_blink = sample_led(L1);
    let p1_leds = led_text();

    press(S5);
    let p2_blink = sample_led(L2);

    press(S5);
    let p3_blink = sample_led(L3);
    let p3_leds = led_text();

    assert_eq(freq_leds, "10000000", "界面指示灯: 频率界面只应点亮 L1");
    assert_eq(humidity_leds, "01000000", "界面指示灯: 湿度界面只应点亮 L2");
    assert_eq(ranging_leds, "00100000", "界面指示灯: 测距界面只应点亮 L3");
    assert(count_edges(p1_blink) >= 4, "参数界面: 频率参数界面 L1 应按 0.1s 闪烁, actual=" + p1_blink);
    assert(count_edges(p2_blink) >= 4, "参数界面: 湿度参数界面 L2 应按 0.1s 闪烁, actual=" + p2_blink);
    assert(count_edges(p3_blink) >= 4, "参数界面: 距离参数界面 L3 应按 0.1s 闪烁, actual=" + p3_blink);
    assert_eq(p1_leds[1..3], "00", "参数界面: 非对应的界面指示灯应熄灭");
    assert_eq(p3_leds[0..2], "00", "参数界面: 非对应的界面指示灯应熄灭");
    p1_blink + " | " + p3_blink
});

// ---------------------------------------------------------------------------
// 检查点 7: 数据与参数的比较指示, 继电器控制 (4.7, 4.9-4/5/6)
// ---------------------------------------------------------------------------
ckpt(7, "阈值指示与继电器", "频率/湿度/距离超过各自参数时 L4/L5/L6 点亮, 距离超限时继电器吸合", || {
    boot(20000, 3.00, 80);
    let over_leds = led_text();
    let over_relay = relay_on();

    set_frequency_hz(1000);
    set_voltage(AIN3, 1.00);
    set_distance_cm(30);
    wait_measure();
    let under_leds = led_text();
    let under_relay = relay_on();

    assert_eq(over_leds[3], '1', "阈值指示: 频率数据超过频率参数时 L4 应点亮");
    assert_eq(over_leds[4], '1', "阈值指示: 湿度数据超过湿度参数时 L5 应点亮");
    assert_eq(over_leds[5], '1', "阈值指示: 距离数据超过距离参数时 L6 应点亮");
    assert_eq(over_relay, true, "继电器: 测距结果大于距离参数时应吸合");
    assert_eq(under_leds[3], '0', "阈值指示: 频率数据低于频率参数时 L4 应熄灭");
    assert_eq(under_leds[4], '0', "阈值指示: 湿度数据低于湿度参数时 L5 应熄灭");
    assert_eq(under_leds[5], '0', "阈值指示: 距离数据低于距离参数时 L6 应熄灭");
    assert_eq(under_relay, false, "继电器: 测距结果小于距离参数时应断开");
    assert_eq(led_text()[6..8], "00", "指示灯: L7/L8 应保持熄灭");
    over_leds + " -> " + under_leds
});

// ---------------------------------------------------------------------------
// 检查点 8: DAC 输出与湿度数据关系 (4.6)
// ---------------------------------------------------------------------------
ckpt(8, "DAC 输出与湿度数据关系", "直线经过 (湿度参数, 1.0V) 与 (80%RH, 5.0V)", || {
    boot(1000, 3.00, 45);
    // 湿度参数默认为 40: 湿度 60% 时输出 1.0 + (60-40)*4/40 = 3.0V.
    let mv_default = dac_mv();

    set_voltage(AIN3, 4.00);
    wait_measure();
    // 湿度 80% 时输出 5.0V.
    let mv_at_80 = dac_mv();

    // 把湿度参数改为 10 后, 湿度 60% 时输出 1.0 + (60-10)*4/70 = 3.86V.
    set_voltage(AIN3, 3.00);
    wait_measure();
    goto_param(1);
    press(S6);
    press(S6);
    press(S6);
    let param_text = display_text(30);
    press(S4);
    wait_measure();
    let mv_param_10 = dac_mv();

    assert_regex(param_text, "^P2\\s+10$", "DAC 输出: 湿度参数应被改为 10");
    assert_in(mv_default, 2900..=3100, "DAC 输出: 湿度参数 40 与湿度 60 应输出 3.0V");
    assert_in(mv_at_80, 4900..=5000, "DAC 输出: 湿度 80 应输出 5.0V");
    assert_in(mv_param_10, 3760..=3960, "DAC 输出: 湿度参数 10 与湿度 60 应输出约 3.86V");
    mv_default.to_string() + "mV, " + mv_at_80.to_string() + "mV, " + mv_param_10.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 9: 脉冲输出 (4.8)
// ---------------------------------------------------------------------------
ckpt(9, "电机驱动脉冲输出", "频率超过参数时 1KHz 80% 占空比, 否则 1KHz 20%", || {
    boot(20000, 0.0, 45);
    let high = sample_motor();

    set_frequency_hz(1000);
    wait_measure();
    let low = sample_motor();

    let high_on = high[0];
    let low_on = low[0];

    assert_in(high_on, 60..=68, "脉冲输出: 频率超过参数时应输出 80% 占空比, actual=" + high_on + "/80");
    assert_in(low_on, 12..=20, "脉冲输出: 频率低于参数时应输出 20% 占空比, actual=" + low_on + "/80");
    assert(high[1] >= 2, "脉冲输出: 2ms 内应至少出现 2 次电平跳变, actual=" + high[1]);
    high_on.to_string() + "/80, " + low_on.to_string() + "/80"
});

// ---------------------------------------------------------------------------
// 检查点 10: E2PROM 继电器开关次数 (4.10)
// ---------------------------------------------------------------------------
ckpt(10, "E2PROM 继电器开关次数", "每次继电器状态变化计数加 1, 长按 S7 清零", || {
    boot(1000, 0.0, 45);
    let initial = eeprom_byte(0);

    // 距离参数默认 0.6M: 80cm 吸合, 30cm 断开, 共 2 次状态变化.
    set_distance_cm(80);
    wait_measure();
    let after_on = eeprom_byte(0);

    set_distance_cm(30);
    wait_measure();
    let after_off = eeprom_byte(0);

    set_distance_cm(80);
    wait_measure();
    set_distance_cm(30);
    wait_measure();
    let after_four = eeprom_byte(0);

    // 湿度界面长按 S7 超过 1s 后松开清零.
    press(S4);
    set_key(S7, true);
    run_ms(1200);
    set_key(S7, false);
    wait_key_response();
    run_ms(200);
    let after_clear = eeprom_byte(0);

    assert_eq(initial, 0, "E2PROM 计数: 初始计数应为 0");
    assert_eq(after_on, 1, "E2PROM 计数: 继电器吸合后计数应为 1");
    assert_eq(after_off, 2, "E2PROM 计数: 继电器断开后计数应为 2");
    assert_eq(after_four, 4, "E2PROM 计数: 四次状态变化后计数应为 4");
    assert_eq(after_clear, 0, "E2PROM 计数: 湿度界面长按 S7 后应清零");
    initial.to_string() + " -> " + after_four.to_string() + " -> " + after_clear.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 11: 响应时间要求
// ---------------------------------------------------------------------------
ckpt(11, "响应时间要求", "按键动作响应 <= 0.2s, 指示灯动作响应 <= 0.1s", || {
    boot(2000, 2.0, 45);

    // 按键响应: 按 S4 由频率界面切到湿度界面, 应在 0.2 秒内完成.
    tap_key(S4, 40);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..1] != "H" {
        run_ms(10);
        key_ms += 10;
    }
    let page = display_text(30);

    // 指示灯响应: 切到湿度界面后, 界面指示灯 L2 应在 0.1 秒内点亮.
    let led_ms = 0;
    while led_ms < 1000 && !led_on(L2) {
        run_ms(10);
        led_ms += 10;
    }
    let leds = led_text();

    assert_in(key_ms, 0..=200, "响应时间: 按键响应应在 0.2 秒内, 实际 " + key_ms.to_string() + "ms, 页面 " + page);
    assert_in(led_ms, 0..=100, "响应时间: 指示灯动作应在 0.1 秒内, 实际 " + led_ms.to_string() + "ms, 指示灯 " + leds);
    key_ms.to_string() + "ms, " + led_ms.to_string() + "ms"
});
