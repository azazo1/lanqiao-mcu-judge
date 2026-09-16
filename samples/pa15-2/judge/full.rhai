// 第十五届蓝桥杯单片机省赛 (第二次, 大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导:
//   3.2 性能: 按键响应 <= 0.1s, 温度采样 <= 1s, 指示灯动作响应 <= 0.1s.
//   3.3 温度: 最终温度 = DS18B20 测量值 + 温度校准值 (可为负);
//             连续两次采样上升或下降幅度超过 1 摄氏度时触发温度突变报警.
//   3.4 DAC: 温度控制模式下输出与温度成直线 (10 摄氏度对应 2V, 40 摄氏度对应 5V);
//             手动控制模式下输出取决于 DAC 数字量 (0-255).
//   3.5 显示: 温度界面 "C" + 温度整数, DAC 输出控制界面 "A" + DAC 数字量 (3 位),
//             校准值界面 "P" + 校准值 (整数, 可负).
//   3.6 按键: S12 循环切换三个界面; S16/S17 在 DAC 界面加减 5, 在校准值界面加减 1,
//             在温度界面且解锁时控制继电器通断; S13 短按切换 DAC 模式, 长按超过 1.5 秒切换锁定状态;
//             校准值在退出校准值界面时生效; DAC 数字量范围 0-255, 校准值范围 -9 到 9.
//   3.7 继电器: 解锁状态下按键可控, 锁定状态下按键控制无效.
//   3.8 LED: L1 温度控制模式, L2 温度上升报警 (亮 2 秒), L3 温度下降报警 (亮 2 秒),
//             L4 温度突变报警 (0.2 秒间隔闪烁, 持续 3 秒), L8 解锁状态.
//   四 初始状态: 温度界面, 继电器解锁, DAC 温度控制模式, DAC 数字量 100, 校准值 0.

key_mode(KBD);

fn wait_measure() {
    // DS18B20 一次转换与刷新约 1 秒 (题目要求采样 <= 1s).
    run_ms(2000);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(300);
}

fn boot(temp) {
    reset(POWER_RESET);
    key_mode(KBD);
    set_temperature_c(temp);
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

// 采样某个指示灯 600ms, 返回电平序列与跳变次数.
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

// 以 100ms 为间隔连续观察指示灯, 用于"闪烁 + 持续一段时间后熄灭"这类要求.
fn sample_led_window(index, steps) {
    let samples = "";
    for step in 1..=steps {
        run_ms(100);
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

// 取显示文本中的整数 (忽略提示符与熄灭位).
fn display_int() {
    let text = display_text(30);
    let digits = "";
    let negative = false;
    for pos in 0..text.len() {
        let ch = text[pos];
        if ch >= '0' && ch <= '9' {
            digits += ch;
        } else if ch == '-' {
            negative = true;
        }
    }
    let value = if digits.len() == 0 { 0 } else { digits.parse_int() };
    if negative { -value } else { value }
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与温度界面 (3.5-1, 3.8, 四)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与温度界面", "温度界面显示整数温度, L1 与 L8 点亮, DAC 输出 3.5V", || {
    boot(25.0);
    let data_text = display_text(30);
    let data_leds = led_text();
    let data_mv = dac_mv();

    assert_regex(data_text, "^C\\s+\\d+$", "温度界面: 应以 C 为提示符显示整数温度");
    assert_eq(display_int(), 25, "温度界面: 25 摄氏度的读数应为 25");
    assert_eq(data_leds[0], '1', "初始状态: DAC 处于温度控制模式时 L1 应点亮");
    assert_eq(data_leds[7], '1', "初始状态: 继电器解锁时 L8 应点亮");
    assert_in(data_mv, 3400..=3600, "DAC 温度控制: 25 摄氏度应输出 3.5V");
    data_text + ", " + data_mv.to_string() + "mV, " + data_leds
});

// ---------------------------------------------------------------------------
// 检查点 2: DAC 温度控制模式的三点 (3.3, 3.4)
// ---------------------------------------------------------------------------
ckpt(2, "DAC 温度控制输出", "10 摄氏度输出 2V, 25 摄氏度输出 3.5V, 40 摄氏度输出 5V", || {
    boot(10.0);
    let low = dac_mv();

    set_temperature_c(25.0);
    wait_measure();
    let mid = dac_mv();

    set_temperature_c(40.0);
    wait_measure();
    let high = dac_mv();

    assert_in(low, 1900..=2100, "DAC 温度控制: 10 摄氏度应输出 2.0V");
    assert_in(mid, 3400..=3600, "DAC 温度控制: 25 摄氏度应输出 3.5V");
    assert_in(high, 4800..=5100, "DAC 温度控制: 40 摄氏度应输出 5.0V");
    low.to_string() + "mV " + mid.to_string() + "mV " + high.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 3: 校准值界面与生效时机 (3.3, 3.5-3, 3.6)
// ---------------------------------------------------------------------------
ckpt(3, "校准值界面与生效时机", "校准值加减 1 并受 -9 到 9 约束, 退出校准值界面后生效", || {
    boot(30.0);
    let before = dac_mv();

    // 校准值加 2 后回到温度界面: 最终温度 32 摄氏度, DAC 应输出 4.2V.
    press(S12);
    press(S12);
    let param_page = display_text(30);
    press(S16);
    press(S16);
    let plus_two = display_int();
    press(S12);
    wait_measure();
    let after = dac_mv();

    // 再把校准值减 4 (总 -2): 最终温度 28 摄氏度, DAC 应输出 3.8V.
    press(S12);
    press(S12);
    for step in 0..4 {
        press(S17);
    }
    let minus_two = display_int();
    press(S12);
    wait_measure();
    let recovered = dac_mv();

    assert_regex(param_page, "^P\\s+[-]?\\d+$", "校准值界面: 应以 P 为提示符显示整数校准值");
    assert_eq(plus_two, 2, "校准值: 加两次后应为 2");
    // DAC 输出量化步进约 20mV, 因此按 ±40mV 判定.
    assert_in(after - before, 160..=240, "校准值生效: 30 摄氏度加 2 度校准后 DAC 应提高约 200mV");
    assert_eq(minus_two, -2, "校准值: 再减 4 后应为 -2");
    assert_in(recovered - before, -240..=-160, "校准值生效: 30 摄氏度减 2 度校准后 DAC 应降低约 200mV");
    plus_two.to_string() + " -> " + after.to_string() + "mV, " + minus_two.to_string() + " -> " + recovered.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 4: DAC 数字量与手动控制模式 (3.4, 3.5-2, 3.6)
// ---------------------------------------------------------------------------
ckpt(4, "DAC 数字量与手动模式", "A 界面显示 DAC 数字量, 手动模式下输出与温度无关", || {
    boot(25.0);
    // 先切到手动控制模式, 此时 A 界面显示的应是可调的 DAC 数字量.
    press(S13);
    press(S12);
    let page = display_text(30);
    let initial = display_int();

    press(S16);
    let up = display_int();
    press(S17);
    press(S17);
    let down = display_int();

    let manual_leds = led_text();
    let manual_mv = dac_mv();
    set_temperature_c(10.0);
    wait_measure();
    let cold_mv = dac_mv();
    set_temperature_c(40.0);
    wait_measure();
    let hot_mv = dac_mv();

    assert_regex(page, "^A\\s+\\d+$", "DAC 输出控制界面: 应以 A 为提示符显示 DAC 数字量");
    assert_eq(initial, 100, "初始状态: DAC 数字量默认值应为 100");
    assert_eq(up, 105, "DAC 数字量: 加一次应为 105");
    assert_eq(down, 95, "DAC 数字量: 连减两次应为 95");
    assert_eq(manual_leds[0], '0', "DAC 模式: 手动控制模式下 L1 应熄灭");
    assert_in(manual_mv, 1800..=2000, "DAC 手动模式: 数字量 95 应输出约 1.86V");
    assert_in((hot_mv - cold_mv).abs(), 0..=30, "DAC 手动模式: 输出不应随温度变化");
    page + " " + initial.to_string() + " -> " + up.to_string() + " -> " + down.to_string() + ", " + manual_mv.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 5: 锁定状态与继电器 (3.6, 3.7, 3.8-3)
// ---------------------------------------------------------------------------
ckpt(5, "锁定状态与继电器", "解锁时按键可控继电器, 长按 S13 锁定后按键控制无效", || {
    boot(25.0);
    // 解锁状态下按 S16 打开继电器, S17 关闭继电器.
    press(S16);
    let opened = relay_on();
    press(S17);
    let closed = relay_on();

    // 长按 S13 超过 1.5 秒进入锁定状态, L8 熄灭.
    set_key(S13, true);
    run_ms(1800);
    set_key(S13, false);
    run_ms(400);
    let locked_leds = led_text();

    press(S16);
    let locked_after_open = relay_on();
    press(S17);
    let locked_after_close = relay_on();

    // 再次长按解锁, 按键恢复控制.
    set_key(S13, true);
    run_ms(1800);
    set_key(S13, false);
    run_ms(400);
    let unlocked_leds = led_text();
    press(S16);
    let unlocked_after_open = relay_on();

    assert_eq(opened, true, "继电器: 解锁状态下 S16 应打开继电器");
    assert_eq(closed, false, "继电器: 解锁状态下 S17 应关闭继电器");
    assert_eq(locked_leds[7], '0', "锁定状态: 锁定后 L8 应熄灭");
    assert_eq(locked_after_open, false, "锁定状态: 锁定后 S16 不应打开继电器");
    assert_eq(locked_after_close, false, "锁定状态: 锁定后 S17 不应改变继电器");
    assert_eq(unlocked_leds[7], '1', "锁定状态: 再长按 S13 后 L8 应重新点亮");
    assert_eq(unlocked_after_open, true, "锁定状态: 解锁后 S16 应恢复打开继电器");
    opened.to_string() + " -> " + closed.to_string() + " -> " + locked_after_open.to_string() + " -> " + unlocked_after_open.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 6: 温度报警指示灯 (3.3, 3.8-2)
// ---------------------------------------------------------------------------
ckpt(6, "温度报警指示灯", "上升 L2 亮 2 秒, 下降 L3 亮 2 秒, 突变时 L4 闪烁 3 秒", || {
    boot(20.0);
    // 上升到 25 摄氏度: 触发上升报警, L2 应点亮并在 2 秒后自动熄灭.
    set_temperature_c(25.0);
    wait_measure();
    let rise_leds = led_text();
    run_ms(2500);
    let rise_after = led_text();

    // 下降到 15 摄氏度: 触发下降报警.
    set_temperature_c(15.0);
    wait_measure();
    let fall_leds = led_text();

    // 突变: 从 15 摄氏度跳到 35 摄氏度 (幅度超过 1 摄氏度), L4 应以 0.2 秒间隔闪烁 3 秒.
    // 固件要先完成一次温度转换才会发现突变, 因此从变化后开始连续观察 6 秒再判定.
    set_temperature_c(35.0);
    let burst = sample_led_window(L4, 60);
    run_ms(3000);
    let burst_after = led_text();

    assert_eq(rise_leds[1], '1', "报警: 温度上升时 L2 应点亮");
    assert_eq(rise_after[1], '0', "报警: 上升报警应在 2 秒后自动熄灭");
    assert_eq(fall_leds[2], '1', "报警: 温度下降时 L3 应点亮");
    assert(burst[1] >= 4, "报警: 温度突变时 L4 应以 0.2 秒间隔闪烁, actual=" + burst[0]);
    assert_eq(burst_after[3], '0', "报警: 突变报警 3 秒后 L4 应熄灭");
    rise_leds + " -> " + rise_after + " -> " + fall_leds + " -> " + burst[0]
});

// ---------------------------------------------------------------------------
// 检查点 7: 响应时间要求 (3.2)
// ---------------------------------------------------------------------------
ckpt(7, "响应时间要求", "按键响应 <= 0.1s, 温度采样 <= 1s, 指示灯动作 <= 0.1s", || {
    boot(24.0);

    // 按键响应: S12 由温度界面切到 DAC 输出控制界面 (标识 A).
    tap_key(S12, 40);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..1] != "A" {
        run_ms(10);
        key_ms += 10;
    }
    let key_page = display_text(30);

    // 指示灯动作: S13 短按切到手动模式, 温度控制指示灯 L1 应很快熄灭.
    tap_key(S13, 40);
    let led_ms = 0;
    while led_ms < 1000 && led_on(L1) {
        run_ms(10);
        led_ms += 10;
    }

    // 温度采样: 环境温度改到 40 度后, 温度界面应在 1 秒内更新 (S12 再按两次回到温度界面).
    press(S12);
    press(S12);
    set_temperature_c(40.0);
    let temp_ms = 0;
    while temp_ms < 3000 && display_number(2, 8) != 40 {
        run_ms(20);
        temp_ms += 20;
    }
    let temp_page = display_text(30);

    assert_in(key_ms, 0..=100, "响应时间: 按键响应应在 0.1 秒内, 实际 " + key_ms.to_string() + "ms, 页面 " + key_page);
    assert_in(led_ms, 0..=100, "响应时间: 指示灯动作应在 0.1 秒内, 实际 " + led_ms.to_string() + "ms");
    assert_in(temp_ms, 0..=1000, "响应时间: 温度采样刷新应在 1 秒内, 实际 " + temp_ms.to_string() + "ms, 页面 " + temp_page);
    key_ms.to_string() + "ms, " + led_ms.to_string() + "ms, " + temp_ms.to_string() + "ms"
});
