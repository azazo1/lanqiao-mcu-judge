// 第十一届蓝桥杯单片机国赛 程序设计试题 (70 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.2 性能: 温度刷新 <= 1s, 亮暗状态刷新 <= 0.5s, 按键响应 <= 0.2s.
//   4.3 显示: 时间界面 时-分-秒 (2 位补 0, '-' 分隔), 温度界面 "C" + 1 位小数,
//             亮暗界面 "E" + 分压结果 (2 位小数) + 状态位 (0 亮 1 暗),
//             参数界面 "P" + 编号 1/2/3 + 参数值 (小时 00-23, 温度 00-99, 指示灯 4-8).
//   4.4 按键: S4 切换数据/参数界面, S5 在两个界面内循环切换子界面, S8/S9 加减当前参数,
//             参数在退出参数界面时生效.
//   4.5 LED: L1 表示当前时间处于 小时参数 至下一个 08:00:00 之间,
//             L2 表示温度数据小于温度参数, L3 表示暗状态持续超过 3 秒,
//             指示灯参数指定的 LED 在暗状态点亮, 其余 L4-L8 熄灭.
//   4.6 初始状态: 数据界面显示时间数据, RTC 默认 16:59:50, 参数默认 17/25/4.

key_mode(KEYBOARD);

// 温度刷新 1s, 亮暗状态刷新 0.5s: 采样等待量取 1.2s.
fn wait_measure() {
    run_ms(1200);
}

// 按键响应 <= 0.2s.
fn press(key) {
    tap_key(key, 60);
    run_ms(200);
}

// 上电并建立确定的初始状态: 时间与光敏分压按参数给定.
fn boot(hour, minute, second, temp, light_mv) {
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    run_ms(1200);
    set_rtc(hour, minute, second);
    set_temperature_c(temp);
    set_voltage(AIN1, light_mv);
    wait_measure();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// 参数界面导航: 从数据界面按 S4 进入参数界面, 再用 S5 选择第 select 个参数 (0/1/2).
fn goto_param(select) {
    press(S4);
    for step in 0..select {
        press(S5);
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与数据界面 (4.3-1, 4.6)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态与时间界面", "数据界面显示 16:59 的时间, 参数默认 17/25/4", || {
    boot(16, 59, 50, 33.2, 0.32);
    let time_text = display_text(30);

    press(S4);
    let p1 = display_text(30);
    press(S5);
    let p2 = display_text(30);
    press(S5);
    let p3 = display_text(30);

    assert_regex(time_text, "^16-59-\\d\\d$", "初始状态: 数据界面应显示 16:59:50 之后的时间数据");
    assert_regex(p1, "^P\\s*1\\s+17$", "初始状态: 时间参数应为 17");
    assert_regex(p2, "^P\\s*2\\s+25$", "初始状态: 温度参数应为 25");
    assert_regex(p3, "^P\\s*3\\s+4$", "初始状态: 指示灯参数应为 4");
    time_text + ", " + p1 + ", " + p2 + ", " + p3
});

// ---------------------------------------------------------------------------
// 检查点 2: 数据界面子界面切换 (4.2-1, 4.3-1)
// ---------------------------------------------------------------------------
ckpt(2, "数据界面切换", "S5 依次显示 时间 -> 温度 -> 亮暗 -> 时间, 格式与读数正确", || {
    boot(16, 59, 50, 33.2, 0.32);
    let time_text = display_text(30);

    press(S5);
    let temp_text = display_text(30);
    let temp_value = (display_number(2, 8) * 10.0).round().to_int();

    press(S5);
    let light_text = display_text(30);

    press(S5);
    let back_text = display_text(30);

    assert_regex(time_text, "^\\d\\d-\\d\\d-\\d\\d$", "数据界面: 时间界面格式错误");
    assert_regex(temp_text, "^C\\s+\\d+\\.\\d$", "数据界面: 温度界面应保留 1 位小数");
    assert_in(temp_value, 331..=333, "数据界面: 33.2 度读数超出容差");
    assert_regex(light_text, "^E\\s+\\d\\.\\d\\d\\s+[01]$", "数据界面: 亮暗界面应显示分压结果与状态位");
    assert_eq(back_text[0..2], "16", "数据界面: 第三次切换应回到时间数据");
    time_text + " -> " + temp_text + " -> " + light_text
});

// ---------------------------------------------------------------------------
// 检查点 3: 参数界面与边界 (4.3-2, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(3, "参数界面与加减边界", "S5 切换三组参数, S8/S9 加减并受范围约束", || {
    boot(16, 59, 50, 33.2, 0.32);
    goto_param(0);
    let p1 = display_text(30);
    press(S9);
    let p1_up = display_text(30);
    press(S8);
    press(S8);
    let p1_down = display_text(30);

    press(S5);
    let p2 = display_text(30);
    press(S9);
    let p2_up = display_text(30);

    press(S5);
    let p3 = display_text(30);
    press(S8);
    let p3_down = display_text(30);

    assert_regex(p1, "^P\\s*1\\s+17$", "参数界面: 时间参数初值应为 17");
    assert_regex(p1_up, "^P\\s*1\\s+18$", "参数界面: 时间参数加 1 后应为 18");
    assert_regex(p1_down, "^P\\s*1\\s+16$", "参数界面: 时间参数连减两次后应为 16");
    assert_regex(p2, "^P\\s*2\\s+25$", "参数界面: 温度参数初值应为 25");
    assert_regex(p2_up, "^P\\s*2\\s+26$", "参数界面: 温度参数加 1 后应为 26");
    assert_regex(p3, "^P\\s*3\\s+4$", "参数界面: 指示灯参数初值应为 4");
    assert_regex(p3_down, "^P\\s*3\\s+4$", "参数界面: 指示灯参数下限为 4, 不能再减");
    p1 + ", " + p2 + ", " + p3
});

// ---------------------------------------------------------------------------
// 检查点 4: 亮暗状态与指示灯 (4.3-1, 4.5-3/4)
// ---------------------------------------------------------------------------
ckpt(4, "亮暗状态与指示灯", "低分压判为暗状态, L3 在暗持续 3s 后点亮, 指定 LED 点亮", || {
    // 分压 0.32V 按图 4 对应暗状态 (状态位为 1).
    boot(16, 59, 50, 33.2, 0.32);
    press(S5);
    press(S5);
    let dark_text = display_text(30);

    // 暗状态持续 3 秒后 L3 应点亮, 指示灯参数 4 指定的 L4 也应点亮.
    run_ms(3500);
    let dark_leds = led_text();

    // 改为亮状态 (分压 4.0V), 持续 3 秒后 L3 与 L4 应熄灭.
    set_voltage(AIN1, 4.0);
    run_ms(3500);
    let bright_text = display_text(30);
    let bright_leds = led_text();

    assert_regex(dark_text, "^E\\s+\\d\\.\\d\\d\\s+1$", "亮暗状态: 0.32V 应按图 4 判为暗状态");
    assert_eq(dark_leds[2], '1', "指示灯: 暗状态持续 3 秒后 L3 应点亮");
    assert_eq(dark_leds[3], '1', "指示灯: 暗状态下指示灯参数指定的 L4 应点亮");
    assert_regex(bright_text, "^E\\s+\\d\\.\\d\\d\\s+0$", "亮暗状态: 4.0V 应判为亮状态");
    assert_eq(bright_leds[2], '0', "指示灯: 亮状态持续 3 秒后 L3 应熄灭");
    assert_eq(bright_leds[3], '0', "指示灯: 亮状态下指定的 L4 应熄灭");
    dark_text + " -> " + bright_text
});

// ---------------------------------------------------------------------------
// 检查点 5: 时间窗口指示与温度指示 (4.5-1/2)
// ---------------------------------------------------------------------------
ckpt(5, "时间窗口与温度指示", "L1 按 小时参数 至次日 08:00 判定, L2 按温度与参数判定", || {
    // 时间参数 17, 当前 20:20: 处于 17:00 至次日 08:00 之间, L1 应点亮.
    boot(20, 20, 0, 33.2, 4.0);
    let inside = led_text();

    // 当前 09:00: 已经过了 08:00, L1 应熄灭.
    set_rtc(9, 0, 0);
    run_ms(1200);
    let outside = led_text();

    // 温度 33.2 度高于温度参数 25: L2 应熄灭; 把温度降到 20 度后 L2 应点亮.
    set_temperature_c(20.0);
    run_ms(1200);
    let cold = led_text();

    assert_eq(inside[0], '1', "时间指示: 20:20 处于时间参数 17 至次日 08:00 之间时 L1 应点亮");
    assert_eq(outside[0], '0', "时间指示: 09:00 已过 08:00 时 L1 应熄灭");
    assert_eq(inside[1], '0', "温度指示: 温度高于温度参数时 L2 应熄灭");
    assert_eq(cold[1], '1', "温度指示: 温度低于温度参数时 L2 应点亮");
    inside + " -> " + outside + " -> " + cold
});
