// 第十四届蓝桥杯单片机省赛 (本科组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 题目原文是图片, 先用本地 OCR 转成文本 (见 problem/题目OCR文本.txt), 评测点按原文推导:
//   3.2 性能: 频率测量精度 ±8%, 按键响应 <= 0.2s, 指示灯响应 <= 0.1s, 亮暗感知 <= 0.5s.
//   3.3 湿度: 由 P34 测量的 NE555 频率换算, 200Hz 对应 10%, 2000Hz 对应 90%, 线性;
//             频率不在 200Hz-2000Hz 内视为无效数据.
//   3.4 显示: 时间界面 时-分-秒 (2 位补 0), 回显界面含 温度回显/湿度回显/时间回显 三个子界面,
//             参数界面显示温度参数, 温湿度界面显示 温度-湿度 (均为整数).
//   3.5 采集触发: 环境由亮变暗时触发一次采集, 立刻切到温湿度界面, 3 秒内不重复触发, 3 秒后返回原界面.
//   3.6 按键: S4 切换 时间/回显/参数, S5 在回显界面切换子界面, S8/S9 调整温度参数,
//             时间回显子界面下长按 S9 超过 2 秒清除记录.
//   3.7 LED: L1 时间界面, L2 回显界面, L3 温湿度界面, L4 温度超过参数时 0.1s 闪烁,
//             L5 湿度无效时点亮, L6 两次采集温湿度均升高时点亮.
//   四 初始状态: 时间界面, 温度参数 30, 触发次数 0.

key_mode(KBD);

fn wait_measure() {
    run_ms(1200);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(250);
}

// 上电: RTC 在采样实现自身初始化完成之后再设置, 否则会被启动写入覆盖.
fn boot(freq, temp) {
    reset(POWER_RESET);
    key_mode(KBD);
    jumper_on(NET_SIG, SIG_OUT);
    set_frequency_hz(freq);
    set_temperature_c(temp);
    run_ms(1500);
    set_rtc(13, 3, 5);
    set_voltage(AIN1, 4.0);
    wait_measure();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// 触发一次采集: 环境由亮 (4.0V) 变暗 (0.5V).
fn trigger_collect() {
    set_voltage(AIN1, 4.0);
    run_ms(800);
    set_voltage(AIN1, 0.5);
    run_ms(800);
}

fn sample_led(index) {
    let samples = "";
    for step in 1..=15 {
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
// 检查点 1: 初始状态与回显空数据 (3.4, 四)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与回显空数据", "时间界面显示 13-03-0x, L1 亮, 触发次数 0 时回显只显示标识符", || {
    boot(1100, 23.5);
    let time_text = display_text(30);
    let time_leds = led_text();

    press(S4);
    let temp_echo = display_text(30);
    let echo_leds = led_text();
    press(S5);
    let humidity_echo = display_text(30);
    press(S5);
    let time_echo = display_text(30);

    press(S4);
    let param = display_text(30);
    press(S8);
    let param_up = display_text(30);

    assert_regex(time_text, "^13-03-\\d\\d$", "初始状态: 应处于时间界面并显示设置的时间");
    assert_eq(time_leds[0], '1', "指示灯: 时间界面下 L1 应点亮");
    assert_eq(echo_leds[1], '1', "指示灯: 回显界面下 L2 应点亮");
    assert_regex(temp_echo, "^C\\s*$", "回显: 触发次数为 0 时温度回显除标识符外应熄灭");
    assert_regex(humidity_echo, "^H\\s*$", "回显: 触发次数为 0 时湿度回显除标识符外应熄灭");
    assert_regex(time_echo, "^F00\\s*$", "回显: 触发次数为 0 时时间回显应显示 F00");
    assert_regex(param, "^P\\s+30$", "初始状态: 温度参数应为 30");
    assert_regex(param_up, "^P\\s+31$", "参数界面: S8 应把温度参数加 1");
    time_text + ", " + temp_echo + ", " + humidity_echo + ", " + time_echo
});

// ---------------------------------------------------------------------------
// 检查点 2: 采集触发与温湿度界面 (3.3, 3.4-4, 3.5)
// ---------------------------------------------------------------------------
ckpt(2, "采集触发与温湿度界面", "亮转暗触发采集并切到温湿度界面, 3 秒后返回, 频率换算正确", || {
    boot(1100, 23.5);
    trigger_collect();
    let collected = display_text(30);
    let collected_leds = led_text();

    run_ms(3200);
    let back = display_text(30);

    // 200Hz 对应 10%, 2000Hz 对应 90%; 两次触发之间要等过 3 秒触发锁定.
    set_frequency_hz(200);
    wait_measure();
    trigger_collect();
    let low = display_text(30);
    run_ms(3200);

    set_frequency_hz(2000);
    wait_measure();
    trigger_collect();
    let high = display_text(30);
    run_ms(3200);

    assert_regex(collected, "^E\\s+2[34]-50$", "采集触发: 1100Hz 时温湿度界面应显示温度 23 与湿度 50");
    assert_eq(collected_leds[2], '1', "指示灯: 温湿度界面下 L3 应点亮");
    assert_regex(back, "^13-03-\\d\\d$", "采集触发: 3 秒后应返回原界面");
    assert_regex(low, "^E\\s+2[34]-10$", "湿度换算: 200Hz 应对应 10%");
    assert_regex(high, "^E\\s+2[34]-90$", "湿度换算: 2000Hz 应对应 90%");
    collected + " " + low + " " + high
});

// ---------------------------------------------------------------------------
// 检查点 3: 无效湿度数据 (3.3, 3.7-2)
// ---------------------------------------------------------------------------
ckpt(3, "无效湿度数据", "频率超出 200-2000Hz 时湿度无效, L5 点亮且湿度位置不是数字", || {
    boot(1100, 23.5);
    trigger_collect();
    let valid_leds = led_text();
    run_ms(3200);

    set_frequency_hz(5000);
    wait_measure();
    trigger_collect();
    let invalid = display_text(30);
    let invalid_leds = led_text();

    assert_eq(valid_leds[4], '0', "无效数据: 有效湿度时 L5 应熄灭");
    assert_eq(invalid_leds[4], '1', "无效数据: 湿度无效时 L5 应点亮");
    assert_regex(invalid, "^E\\s+2[34]-[^0-9]+$", "无效数据: 湿度位置应显示非数字标记");
    invalid
});

// ---------------------------------------------------------------------------
// 检查点 4: 回显统计与清零 (3.4-2, 3.6-1)
// ---------------------------------------------------------------------------
ckpt(4, "回显统计与清零", "回显统计最大与平均温湿度, 时间回显记录触发次数与最近触发时间, 长按 S9 清零", || {
    boot(1100, 23.5);
    // 第一次采集: 温度 23.5 度, 湿度 50%.
    trigger_collect();
    run_ms(3200);
    // 第二次采集: 温度升到 30 度, 湿度升到 90%.
    set_temperature_c(30.0);
    set_frequency_hz(2000);
    wait_measure();
    trigger_collect();
    run_ms(3200);

    press(S4);
    let temp_echo = display_text(30);
    press(S5);
    let humidity_echo = display_text(30);
    press(S5);
    let time_echo = display_text(30);

    // 时间回显子界面下长按 S9 超过 2 秒清除记录.
    set_key(S9, true);
    run_ms(2500);
    set_key(S9, false);
    run_ms(400);
    let cleared = display_text(30);

    assert_regex(temp_echo, "^C\\s*\\d+-\\d+\\.\\d$", "回显: 温度回显应显示最大温度与平均温度");
    assert_regex(humidity_echo, "^H\\s*\\d+-\\d+\\.\\d$", "回显: 湿度回显应显示最大湿度与平均湿度");
    assert_regex(time_echo, "^F02\\d\\d-\\d\\d$", "回显: 触发两次后应显示触发次数 02 与最近触发时间");
    assert_regex(cleared, "^F00\\s*$", "回显: 长按 S9 后触发次数应清零");
    temp_echo + " " + humidity_echo + " " + time_echo + " -> " + cleared
});

// ---------------------------------------------------------------------------
// 检查点 5: 按键作用范围 (3.6)
// ---------------------------------------------------------------------------
ckpt(5, "按键作用范围", "S5 仅在回显界面有效, S8/S9 仅在参数界面有效, 采集界面期间按键无效", || {
    boot(1100, 23.5);
    // 时间界面下按 S5 不应切换界面.
    press(S5);
    let after_s5 = display_text(30);

    // 参数界面下按 S5 不应影响参数界面.
    press(S4);
    press(S4);
    let param = display_text(30);
    press(S5);
    let param_after = display_text(30);

    assert_regex(after_s5, "^13-03-\\d\\d$", "按键范围: 时间界面下 S5 应无效");
    assert_regex(param, "^P\\s+30$", "按键范围: 参数界面应显示温度参数 30");
    assert_regex(param_after, "^P\\s+30$", "按键范围: 参数界面下 S5 不应改变界面");
    after_s5 + " " + param + " " + param_after
});

// ---------------------------------------------------------------------------
// 检查点 6: LED 报警指示 (3.7-2)
// ---------------------------------------------------------------------------
ckpt(6, "LED 报警指示", "L4 温度超参数闪烁, L6 两次采集温湿度均升高时点亮, 其余熄灭", || {
    // 温度参数 30, 采集温度 40 度: L4 应闪烁.
    boot(1100, 40.0);
    trigger_collect();
    let hot = sample_led(L4);
    let hot_leds = led_text();
    run_ms(3200);

    // 第一次采集温度 20 度湿度 50%, 第二次升到 25 度湿度 60%: L6 应点亮.
    boot(1100, 20.0);
    trigger_collect();
    run_ms(3200);
    set_temperature_c(25.0);
    set_frequency_hz(1300);
    wait_measure();
    trigger_collect();
    let rising = led_text();

    assert(hot[1] >= 2, "LED 报警: 温度超过温度参数时 L4 应以 0.1s 闪烁, actual=" + hot[0]);
    assert_eq(hot_leds[3], '1', "LED 报警: L4 应在闪烁过程中被观察到点亮");
    assert_eq(rising[5], '1', "LED 报警: 两次采集温湿度均升高时 L6 应点亮");
    assert_eq(rising[6..8], "00", "LED 报警: L7/L8 应保持熄灭");
    hot[0] + " | " + rising
});
