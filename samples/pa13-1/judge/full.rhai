// 第十三届蓝桥杯单片机省赛第一场 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.2 性能: 温度刷新 <= 1s, 按键响应 <= 0.2s, 继电器响应 <= 0.1s.
//   4.3 显示: U1 界面编号 + 温度 (1 位小数), U2 界面编号 + 时/分 (24 小时制),
//             U3 界面编号 + 温度参数.
//   4.4 按键: S12 循环切换三个界面, S13 切换工作模式, S16 参数加 1, S17 参数减 1,
//             S17 在时间界面按住时显示 分-秒, 参数范围 10-99.
//   4.5 继电器: 温度控制模式按温度与参数比较, 时间控制模式在每个整点吸合 5 秒.
//   4.6 LED: L1 整点后点亮 5 秒, L2 表示温度控制模式, L3 在继电器吸合时按 0.1s 闪烁.
//   4.7 初始状态: 温度界面, 温度控制模式, 参数 23 度, 蜂鸣器静音.
//
// 显示格式按题图理解: "U1" 后面是若干熄灭位, 数值右对齐; 时/分界面用 '-' 作为分隔符.
// 采样实现的 DS1302 会在启动时写入初始时间, 因此评测脚本在启动初始化完成后再设置 RTC,
// 并把时间设到非整点, 避免整点事件干扰其它评测点.

key_mode(KEYBOARD);

// 温度刷新窗口: 题目要求 <= 1s; 继电器在温度条件成立后 <= 0.1s 动作,
// 因此温度类断言的等待量取 1s + 0.1s 再加一点采样余量.
fn wait_relay_by_temp() {
    run_ms(1100);
}

// 等待一个按键响应窗口 (题目要求 <= 0.2s).
fn wait_key_response() {
    run_ms(200);
}

fn press(key) {
    tap_key(key, 60);
    wait_key_response();
}

// 连续按键, 用于参数边界测试; 这里不测试响应时间, 所以等待量取小一些.
fn press_burst(key, times) {
    for step in 0..times {
        tap_key(key, 40);
        run_ms(60);
    }
}

// 上电并完成启动初始化: 温度按参数传入, RTC 设到 08:30:20 (非整点).
fn boot(temp) {
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    set_temperature_c(temp);
    run_ms(1200);
    set_rtc(8, 30, 20);
    run_ms(400);
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// 观察一段时间内 L3 的电平序列, 用于判断继电器吸合时是否在闪烁.
fn sample_l3(window_ms, step_ms) {
    let samples = "";
    let elapsed = 0;
    while elapsed < window_ms {
        run_ms(step_ms);
        elapsed += step_ms;
        samples += if led_on(L3) { "1" } else { "0" };
    }
    samples
}

// 温度显示保留 1 位小数, 用 "分摄氏度" 整数做容差比较.
fn temp_dc(value) {
    (value * 10.0).round().to_int()
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态 (4.7, 4.5-1, 4.6)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态", "温度界面, 温度控制模式, 参数 23, 继电器断开", || {
    // 环境温度低于参数, 温度控制模式下继电器应保持断开.
    boot(20.0);
    let text = display_text(30);
    let leds = led_text();
    let relay = relay_on();
    let buzzer = buzzer_on();

    assert_regex(text, "^U1\\s+20\\.0$", "初始状态: 应处于温度界面并显示 20.0");
    assert_eq(leds[1], '1', "初始状态: 温度控制模式下 L2 应点亮");
    assert_eq(leds[0], '0', "初始状态: 非整点时 L1 应熄灭");
    assert_eq(leds[2], '0', "初始状态: 继电器断开时 L3 应熄灭");
    assert_eq(leds[3..8], "00000", "初始状态: L4-L8 应保持熄灭");
    assert_eq(relay, false, "初始状态: 温度低于参数时继电器应断开");
    assert_eq(buzzer, false, "初始状态: 蜂鸣器应保持静音");
    text
});

// ---------------------------------------------------------------------------
// 检查点 2: 温度显示界面 (4.3-1, 4.2-1)
// ---------------------------------------------------------------------------
ckpt(2, "温度显示与刷新", "U1 显示 1 位小数温度, 1s 内刷新", || {
    boot(20.0);

    set_temperature_c(23.5);
    run_ms(1000);
    let text_23_5 = display_text(30);
    let value_23_5 = temp_dc(display_number(3, 8));

    set_temperature_c(26.8125);
    run_ms(1000);
    let text_26_8 = display_text(30);
    let value_26_8 = temp_dc(display_number(3, 8));

    assert_regex(text_23_5, "^U1\\s+23\\.5$", "温度界面: 应显示 23.5");
    assert_in(value_23_5, 234..=236, "温度界面: 23.5 度读数超出容差");
    assert_regex(text_26_8, "^U1\\s+26\\.8$", "温度界面: 应显示 26.8");
    assert_in(value_26_8, 267..=269, "温度界面: 26.8 度读数超出容差");
    text_23_5 + " -> " + text_26_8
});

// ---------------------------------------------------------------------------
// 检查点 3: 时间显示界面与 S17 按住显示分秒 (4.3-2, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(3, "时间界面与 S17 按住显示分秒", "显示 时-分, 按住 S17 显示 分-秒", || {
    boot(20.0);
    press(S12);

    set_rtc(13, 45, 10);
    run_ms(400);
    let hour_minute = display_text(30);

    // 按住 S17: 时间界面应改为显示 分-秒.
    set_key(S17, true);
    run_ms(300);
    let minute_second = display_text(30);

    // 松开 S17: 恢复显示 时-分.
    set_key(S17, false);
    run_ms(300);
    let restored = display_text(30);
    let leds = led_text();

    assert_regex(hour_minute, "^U2 13-45$", "时间界面: 24 小时制应显示 13-45");
    assert_regex(minute_second, "^U2 45-\\d\\d$", "时间界面: 按住 S17 应显示 分-秒");
    assert_regex(restored, "^U2 13-45$", "时间界面: 松开 S17 应恢复 时-分");
    assert_eq(leds[1], '1', "时间界面: 温度控制模式下 L2 仍应点亮");
    hour_minute + " -> " + minute_second + " -> " + restored
});

// ---------------------------------------------------------------------------
// 检查点 4: 参数界面与 S16/S17 作用范围 (4.3-3, 4.4-1, 4.4-2)
// ---------------------------------------------------------------------------
ckpt(4, "参数界面与按键作用范围", "S16 加 1, S17 减 1, 温度界面下 S17 无效", || {
    boot(20.0);
    press(S12);
    press(S12);
    let initial = display_text(30);

    press(S16);
    let after_up = display_text(30);
    press(S17);
    let after_down = display_text(30);

    // 回到温度界面, S17 在温度界面不应有任何效果.
    press(S12);
    let temp_page = display_text(30);
    set_key(S17, true);
    run_ms(300);
    let temp_page_holding = display_text(30);
    set_key(S17, false);
    run_ms(300);

    // 回到参数界面确认参数没有被改动.
    press(S12);
    press(S12);
    let param_again = display_text(30);

    assert_regex(initial, "^U3\\s+23$", "参数界面: 上电参数应为 23");
    assert_regex(after_up, "^U3\\s+24$", "参数界面: S16 后参数应为 24");
    assert_regex(after_down, "^U3\\s+23$", "参数界面: S17 后参数应为 23");
    assert_regex(temp_page, "^U1\\s+20\\.0$", "参数界面: S12 应切回温度界面");
    assert_eq(temp_page_holding, temp_page, "按键作用范围: 温度界面按住 S17 不应改变显示");
    assert_regex(param_again, "^U3\\s+23$", "按键作用范围: 温度界面下 S17 不应改动参数");
    param_again
});

// ---------------------------------------------------------------------------
// 检查点 5: 温度参数范围 (4.4-2)
// ---------------------------------------------------------------------------
ckpt(5, "温度参数调整范围", "参数可调到 10 与 99 并在此范围内夹紧", || {
    boot(20.0);
    press(S12);
    press(S12);

    // 从 23 连续减到下限.
    press_burst(S17, 20);
    let low = display_text(30);
    press_burst(S17, 5);
    let low_again = display_text(30);

    // 从下限连续加到上限.
    press_burst(S16, 95);
    let high = display_text(30);
    press_burst(S16, 5);
    let high_again = display_text(30);

    assert_regex(low, "^U3\\s+10$", "参数范围: 连续减应夹紧在 10");
    assert_regex(low_again, "^U3\\s+10$", "参数范围: 到达下限后继续减应保持 10");
    assert_regex(high, "^U3\\s+99$", "参数范围: 连续加应夹紧在 99");
    assert_regex(high_again, "^U3\\s+99$", "参数范围: 到达上限后继续加应保持 99");
    low + " -> " + high
});

// ---------------------------------------------------------------------------
// 检查点 6: 温度控制模式下的继电器与 L3 (4.5-1, 4.2-3, 4.6-3)
// ---------------------------------------------------------------------------
ckpt(6, "温度控制模式继电器", "温度超过参数吸合, 低于参数断开, 吸合时 L3 闪烁", || {
    boot(20.0);

    // 参数 23, 温度升到 24.0 后继电器应在温度刷新 + 0.1s 内吸合.
    set_temperature_c(24.0);
    wait_relay_by_temp();
    let relay_hot = relay_on();
    let blink_hot = sample_l3(400, 40);
    let leds_hot = led_text();

    set_temperature_c(22.0);
    wait_relay_by_temp();
    let relay_cold = relay_on();
    let leds_cold = led_text();

    assert_eq(relay_hot, true, "温度控制: 温度超过参数时继电器应吸合");
    assert_eq(blink_hot.contains("1"), true, "温度控制: 继电器吸合时 L3 应有点亮相位");
    assert_eq(blink_hot.contains("0"), true, "温度控制: 继电器吸合时 L3 应有熄灭相位");
    assert_eq(leds_hot[2], '1', "温度控制: 继电器吸合时 L3 应参与闪烁");
    assert_eq(relay_cold, false, "温度控制: 温度低于参数时继电器应断开");
    assert_eq(leds_cold[2], '0', "温度控制: 继电器断开后 L3 应熄灭");
    blink_hot
});

// ---------------------------------------------------------------------------
// 检查点 7: 时间控制模式与整点动作 (4.5-2, 4.6-1)
// ---------------------------------------------------------------------------
ckpt(7, "时间控制模式整点动作", "整点后 0.2s 内吸合, 吸合 5s 后断开, L1 同步", || {
    boot(20.0);
    press(S13);
    let mode_leds = led_text();

    // 把 RTC 设到 07:59:59, 从设置时刻起 1s 后就是整点; 按 10ms 步长等待继电器吸合,
    // 记录相对整点的延迟. 脚本无法直接读 DS1302 的内部秒边界, 因此延迟窗口取
    // 题目的 0.1s 加上一个采样周期的相位余量.
    set_rtc(7, 59, 59);
    let latency_ms = -1;
    let elapsed_ms = 0;
    while elapsed_ms < 1500 {
        run_ms(10);
        elapsed_ms += 10;
        if relay_on() && latency_ms < 0 {
            latency_ms = elapsed_ms - 1000;
            break;
        }
    }
    let relay_at_hour = relay_on();
    let leds_at_hour = led_text();

    // 吸合后 4.9s: 仍未到 5s, 应保持吸合; 再等 0.2s 应断开.
    run_ms(4900);
    let relay_before_release = relay_on();
    let leds_before_release = led_text();

    run_ms(200);
    let relay_after_release = relay_on();
    let leds_after_release = led_text();

    assert_eq(mode_leds[1], '0', "时间控制模式: L2 应熄灭");
    assert_in(latency_ms, 0..=200, "整点动作: 整点后继电器吸合延迟超出 0.1s 要求");
    assert_eq(relay_at_hour, true, "整点动作: 整点后继电器应吸合");
    assert_eq(leds_at_hour[0], '1', "整点动作: 整点时 L1 应点亮");
    assert_eq(relay_before_release, true, "整点动作: 吸合后 4.9s 仍应保持吸合");
    assert_eq(leds_before_release[0], '1', "整点动作: 5s 内 L1 应保持点亮");
    assert_eq(relay_after_release, false, "整点动作: 吸合 5s 后继电器应断开");
    assert_eq(leds_after_release[0], '0', "整点动作: 5s 后 L1 应熄灭");
    "延迟 " + latency_ms + "ms, " + leds_at_hour + " -> " + leds_after_release
});

// ---------------------------------------------------------------------------
// 检查点 8: 其余指示灯与蜂鸣器 (4.6-4, 4.7)
// ---------------------------------------------------------------------------
ckpt(8, "其余指示灯与蜂鸣器", "L4-L8 保持熄灭, 蜂鸣器静音", || {
    boot(20.0);
    let leds_initial = led_text();

    set_temperature_c(30.0);
    wait_relay_by_temp();
    let leds_relay = led_text();

    press(S12);
    press(S12);
    let leds_param = led_text();
    let buzzer = buzzer_on();

    assert_eq(leds_initial[3..8], "00000", "指示灯: 初始状态下 L4-L8 应熄灭");
    assert_eq(leds_relay[3..8], "00000", "指示灯: 继电器吸合时 L4-L8 应熄灭");
    assert_eq(leds_param[3..8], "00000", "指示灯: 参数界面下 L4-L8 应熄灭");
    assert_eq(buzzer, false, "蜂鸣器: 整个运行过程应保持静音");
    leds_relay
});

// ---------------------------------------------------------------------------
// 检查点 9: 响应时间要求
// ---------------------------------------------------------------------------
ckpt(9, "响应时间要求", "按键动作响应 <= 0.2s, 继电器响应 <= 0.1s", || {
    // 温度设成高于参数, 让温度控制模式下的继电器处于吸合条件.
    boot(40.0);

    // 按键响应: 按 S12 由温度界面切到时间界面, 应在 0.2 秒内完成.
    tap_key(S12, 40);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10).contains("-") == false {
        run_ms(10);
        key_ms += 10;
    }
    let time_page = display_text(30);

    // 继电器响应: 条件满足后继电器应在 0.1 秒内吸合.
    let relay_ms = 0;
    while relay_ms < 1000 && !relay_on() {
        run_ms(10);
        relay_ms += 10;
    }
    let relay_state = relay_on();

    assert_in(key_ms, 0..=200, "响应时间: 按键响应应在 0.2 秒内, 实际 " + key_ms.to_string() + "ms, 页面 " + time_page);
    assert_in(relay_ms, 0..=100, "响应时间: 继电器响应应在 0.1 秒内, 实际 " + relay_ms.to_string() + "ms, 状态 " + relay_state.to_string());
    key_ms.to_string() + "ms, " + relay_ms.to_string() + "ms"
});
