// 第十届蓝桥杯单片机国赛 程序设计试题 (70 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.1 性能: 温度刷新 <= 0.5s, 距离刷新 <= 1s, 串口响应 <= 500ms; 测距范围 10cm-50cm, 声速 340m/s.
//   4.2 显示: 数据界面温度 "C" + 2 位小数, 距离 "L" + 2 位, 变更次数 "N" + 5 位;
//             参数界面 "P" + 编号 1/2 + 参数值, 温度参数范围 0-99, 距离参数范围 0-99.
//   4.3 按键: S13 切换数据/参数界面, S12 在两级界面内切换子界面, S16/S17 加减参数
//             (温度 ±2℃, 距离 ±5cm), 长按 S12 (>1s) 重置变动次数, 长按 S13 切换 DAC 启动/停止.
//   4.4 存储: 退出参数界面时若参数发生变化, 变动次数加 1 并写入 E2PROM (范围 0-65535).
//   4.5 串口: 4800bps 无校验 1 位停止位; "ST" 返回 "$距离,温度", "PARA" 返回 "#距离参数,温度参数",
//             错误指令返回 "ERROR".
//   4.6 DAC: 启动状态 S <= 距离参数 输出 2V, S > 距离参数 输出 4V; 停止状态固定 0.4V.
//   4.7 LED: L1 表示温度超参数, L2 表示距离小于参数, L3 表示 DAC 启动.
//   4.8 初始状态: DAC 启动, 数据界面显示温度数据, 温度参数 30, 距离参数 35.

key_mode(KEYBOARD);
uart_config(8, 4800, 1, "none");

// 温度刷新 0.5s, 距离刷新 1s: 测量等待量取 1.2s 覆盖两者.
fn wait_measure() {
    run_ms(1200);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(200);
}

fn long_press(key) {
    set_key(key, true);
    run_ms(1200);
    set_key(key, false);
    run_ms(300);
}

// 上电并建立确定的初始状态: EEPROM 计数清零, 温度与距离按参数给定.
fn boot(temp, distance) {
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    uart_config(8, 4800, 1, "none");
    set_eeprom(0, 0);
    set_eeprom(1, 0);
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

// E2PROM 中的变动次数: 按低字节在前读取 16 位.
fn eeprom_count() {
    eeprom_byte(0) + eeprom_byte(1) * 256
}

fn dac_mv() {
    da_value() * 5000 / 255
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态与温度数据界面 (4.1, 4.2-1, 4.7, 4.8)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态与温度数据", "数据界面显示温度两位小数, DAC 启动, 参数 30/35", || {
    boot(26.32, 30);
    let text = display_text(30);
    let leds = led_text();
    let mv = dac_mv();
    // 数值读取要趁当前界面还是数据界面时进行.
    let temp_value = (display_number(2, 8) * 100.0).round().to_int();

    // 进入参数界面读取两个默认参数.
    press(S13);
    let p1 = display_text(30);
    press(S12);
    let p2 = display_text(30);

    assert_regex(text, "^C\\s+\\d+\\.\\d\\d$", "初始状态: 数据界面应显示温度并保留 2 位小数");
    assert_in(temp_value, 2627..=2637, "温度显示: 26.32 度读数超出容差");
    assert_eq(leds[2], '1', "初始状态: DAC 启动时 L3 应点亮");
    assert_in(mv, 1900..=2100, "DAC 输出: 距离 30cm 小于距离参数 35cm 时应输出 2V");
    assert_regex(p1, "^P\\s+1\\s+30$", "初始状态: 温度参数应为 30");
    assert_regex(p2, "^P\\s+2\\s+35$", "初始状态: 距离参数应为 35");
    text + ", " + p1 + ", " + p2
});

// ---------------------------------------------------------------------------
// 检查点 2: 数据界面子界面切换 (4.2-1, 4.3-1)
// ---------------------------------------------------------------------------
ckpt(2, "数据界面切换", "S12 依次显示 温度 -> 距离 -> 变更次数 -> 温度, 各自格式正确", || {
    boot(26.32, 30);
    let temp_page = display_text(30);

    press(S12);
    let distance_page = display_text(30);
    let distance_value = display_number(2, 8);

    press(S12);
    let count_page = display_text(30);

    press(S12);
    let back_page = display_text(30);

    assert_regex(temp_page, "^C\\s+\\d+\\.\\d\\d$", "数据界面: 温度界面格式错误");
    assert_regex(distance_page, "^L\\s+\\d+$", "数据界面: 距离界面格式错误");
    assert_in(distance_value, 27..=33, "数据界面: 30cm 的距离读数超出 ±3cm");
    assert_regex(count_page, "^N\\s+\\d+$", "数据界面: 变更次数界面应以 N 为提示符并显示 5 位以内整数");
    assert_eq(back_page[0], 'C', "数据界面: 第三次切换应回到温度数据");
    temp_page + " -> " + distance_page + " -> " + count_page
});

// ---------------------------------------------------------------------------
// 检查点 3: 参数界面与加减 (4.2-2, 4.3-1)
// ---------------------------------------------------------------------------
ckpt(3, "参数界面与加减参数", "温度参数 ±2, 距离参数 ±5, 从数据界面进入默认温度参数", || {
    boot(26.32, 30);
    press(S13);
    let p1 = display_text(30);

    press(S17);
    let p1_up = display_text(30);
    press(S16);
    press(S16);
    let p1_down = display_text(30);

    press(S12);
    let p2 = display_text(30);
    press(S17);
    let p2_up = display_text(30);
    press(S16);
    let p2_down = display_text(30);

    assert_regex(p1, "^P\\s+1\\s+30$", "参数界面: 默认应显示温度参数 30");
    assert_regex(p1_up, "^P\\s+1\\s+32$", "参数界面: 温度参数加 2 后应为 32");
    assert_regex(p1_down, "^P\\s+1\\s+28$", "参数界面: 温度参数减 2 两次后应为 28");
    assert_regex(p2, "^P\\s+2\\s+35$", "参数界面: 距离参数默认应为 35");
    assert_regex(p2_up, "^P\\s+2\\s+40$", "参数界面: 距离参数加 5 后应为 40");
    assert_regex(p2_down, "^P\\s+2\\s+35$", "参数界面: 距离参数减 5 后应为 35");
    p1 + ", " + p2_up
});

// ---------------------------------------------------------------------------
// 检查点 4: 参数变动次数与 E2PROM (4.4, 4.3-2)
// ---------------------------------------------------------------------------
ckpt(4, "参数变动次数与 E2PROM", "参数变化并退出后计数加 1 并写入 E2PROM, 长按 S12 清零", || {
    boot(26.32, 30);
    let initial = eeprom_count();

    // 修改温度参数后从参数界面退出.
    press(S13);
    press(S17);
    press(S13);
    let after_change = eeprom_count();

    // 只进入参数界面而不修改, 退出时计数不应变化.
    press(S13);
    press(S13);
    let after_visit = eeprom_count();

    // 长按 S12 重置计数.
    long_press(S12);
    let after_reset = eeprom_count();

    assert_eq(initial, 0, "变动次数: 上电初值应为 0");
    assert_eq(after_change, 1, "变动次数: 参数变化并退出后应加 1");
    assert_eq(after_visit, 1, "变动次数: 参数未变化时退出不应计数");
    assert_eq(after_reset, 0, "变动次数: 长按 S12 后应清零");
    initial.to_string() + " -> " + after_change.to_string() + " -> " + after_reset.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 5: DAC 输出 (4.6, 4.3-2)
// ---------------------------------------------------------------------------
ckpt(5, "DAC 输出", "启动时距离不超过参数输出 2V, 超过输出 4V, 停止时输出 0.4V", || {
    boot(26.32, 30);
    wait_measure();
    let below = dac_mv();

    set_distance_cm(50);
    wait_measure();
    let above = dac_mv();

    long_press(S13);
    let stopped = dac_mv();

    long_press(S13);
    let resumed = dac_mv();

    assert_in(below, 1900..=2100, "DAC 输出: 距离 30cm 不超过距离参数 35cm 时应输出 2V");
    assert_in(above, 3900..=4100, "DAC 输出: 距离 50cm 超过距离参数 35cm 时应输出 4V");
    assert_in(stopped, 380..=420, "DAC 输出: 长按 S13 停止后应输出 0.4V");
    assert_in(resumed, 3900..=4100, "DAC 输出: 再次长按 S13 应恢复启动状态输出");
    below.to_string() + "mV, " + above.to_string() + "mV, " + stopped.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 6: LED 指示灯 (4.7)
// ---------------------------------------------------------------------------
ckpt(6, "LED 指示灯", "L1 温度超参数, L2 距离小于参数, L3 启动状态, 其余熄灭", || {
    boot(26.32, 30);
    let leds_low = led_text();

    // 温度超过温度参数: 温度改为 40 度.
    set_temperature_c(40.0);
    wait_measure();
    let leds_hot = led_text();

    // 距离大于等于距离参数: 距离改为 40cm, 同时温度仍高于参数.
    set_distance_cm(45);
    wait_measure();
    let leds_far = led_text();

    assert_eq(leds_low[1], '1', "LED: 距离小于距离参数时 L2 应点亮");
    assert_eq(leds_low[0], '0', "LED: 温度未超过温度参数时 L1 应熄灭");
    assert_eq(leds_hot[0], '1', "LED: 温度超过温度参数时 L1 应点亮");
    assert_eq(leds_far[1], '0', "LED: 距离超过距离参数时 L2 应熄灭");
    assert_eq(leds_low[2], '1', "LED: DAC 启动时 L3 应点亮");
    assert_eq(leds_far[3..8], "00000", "LED: 其余指示灯应熄灭");
    leds_low + " -> " + leds_far
});

// ---------------------------------------------------------------------------
// 检查点 7: 串口召测 (4.5)
// ---------------------------------------------------------------------------
ckpt(7, "串口召测功能", "ST 返回 $距离,温度, PARA 返回 #距离参数,温度参数, 错误指令返回 ERROR", || {
    boot(26.32, 30);
    // 温度 26.32 度, 距离 30cm, 参数为温度 30 与距离 35.
    let temp_value = (display_number(2, 8) * 100.0).round().to_int();

    uart_write("ST\r\n");
    run_ms(300);
    let data = uart_take(50);

    uart_write("PARA\r\n");
    run_ms(300);
    let param = uart_take(50);

    uart_write("XYZ\r\n");
    run_ms(300);
    let wrong = uart_take(50);

    assert_regex(data, "^\\$\\d+,2[67]\\.\\d\\d", "串口: ST 应返回 $距离,温度");
    assert_in(temp_value, 2627..=2637, "串口: 温度数据应与测量值一致");
    assert_regex(param, "^#35,30", "串口: PARA 应按 距离参数,温度参数 的顺序返回");
    assert_regex(wrong, "^ERROR", "串口: 错误指令应返回 ERROR");
    data + " | " + param + " | " + wrong
});

// ---------------------------------------------------------------------------
// 检查点 8: 刷新与响应时间 (4.1)
// ---------------------------------------------------------------------------
ckpt(8, "刷新与响应时间", "温度刷新 <= 0.5s, 距离刷新 <= 1s, 串口响应 <= 500ms", || {
    boot(26.32, 30);

    // 温度刷新: 环境温度改到 40 度后, 数据界面应在 0.5 秒内更新.
    set_temperature_c(40.0);
    let temp_ms = 0;
    while temp_ms < 3000 && (display_number(2, 8) - 40.0).abs() > 0.1 {
        run_ms(10);
        temp_ms += 10;
    }
    let temp_page = display_text(30);

    // 距离刷新: 距离改到 45cm 后, 距离界面应在 1 秒内更新.
    press(S12);
    set_distance_cm(45);
    let dist_ms = 0;
    while dist_ms < 3000 && (display_number(2, 8) - 45).abs() > 3 {
        run_ms(20);
        dist_ms += 20;
    }
    let dist_page = display_text(30);

    // 串口响应: 发查询指令后应在 500ms 内收到回复.
    uart_write("ST\r\n");
    let uart_ms = 0;
    while uart_ms < 2000 && uart_peek(1).len() == 0 {
        run_ms(10);
        uart_ms += 10;
    }
    let reply = uart_take(80);

    assert_in(temp_ms, 0..=500, "刷新时间: 温度数据应在 0.5 秒内刷新, 实际 " + temp_ms.to_string() + "ms, 页面 " + temp_page);
    assert_in(dist_ms, 0..=1000, "刷新时间: 距离数据应在 1 秒内刷新, 实际 " + dist_ms.to_string() + "ms, 页面 " + dist_page);
    assert_in(uart_ms, 0..=500, "响应时间: 串口指令应在 500ms 内响应, 实际 " + uart_ms.to_string() + "ms, 回复 " + reply);
    temp_ms.to_string() + "ms, " + dist_ms.to_string() + "ms, " + uart_ms.to_string() + "ms"
});
