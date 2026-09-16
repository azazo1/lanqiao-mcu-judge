// 第十二届蓝桥杯单片机国赛 程序设计试题 整卷评测脚本 (超声波物位计).
//
// 评测点全部按题目原文推导:
//   4.2 性能: 界面切换 <= 0.2s, 按键响应 <= 0.2s, 测距范围 >= 80cm,
//             测距精度 <= ±4cm, DAC 输出误差 <= ±0.3V.
//   4.3 显示: 时间界面 时-分-秒, 距离界面 "L" + 模式标识 (触发 C / 定时 F) + 距离整数,
//             数据记录界面 "H" + 类型标识 (a/g/d 段分别表示最大值/平均值/最小值) + 数据,
//             参数界面 "P1" + 采集时间 (取值 2/3/5/7/9s) 与 "P2" + 距离参数 (10-80cm).
//   4.4 按键: S4 切换数据界面与参数界面, S5 在各级界面内切换子界面,
//             S8 在距离界面切换触发/定时模式, 在记录界面按最大值/最小值/平均值切换,
//             S9 循环采集时间参数, 距离参数加 10; 进参数界面默认采集时间,
//             回数据界面默认时间界面, 进记录界面默认最大值; 参数退出参数界面后生效.
//   4.5 DAC: 输出电压与最近一次测距结果相关, 10cm 对应 1.0V, 80cm 对应 5.0V.
//   4.6 采集: 定时模式下由 RTC 秒值整除采集时间参数触发, 触发模式下由环境光由亮变暗触发.
//   4.7 LED: L1 时间界面, L2 距离界面, L3 记录界面, L4 触发模式, L5 定时模式下连续 3 次
//             距离在距离参数 ±5cm 内, L6 环境光为亮.
//   4.8 初始状态: 时间数据显示界面, 触发模式, 采集时间 2s, 距离参数 20cm.

key_mode(KBD);

fn wait_measure() {
    run_ms(1200);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(300);
}

fn boot(distance, light_v, second) {
    reset(POWER_RESET);
    key_mode(KBD);
    set_distance_cm(distance);
    set_voltage(AIN1, light_v);
    run_ms(2000);
    set_rtc(20, 20, second);
    run_ms(1200);
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    return text;
}

fn dac_mv() {
    da_value() * 5000 / 255
}

// 触发一次采集: 环境光由亮 (4.0V) 变暗 (0.5V).
fn trigger_collect() {
    set_voltage(AIN1, 4.0);
    run_ms(600);
    set_voltage(AIN1, 0.5);
    run_ms(600);
}

// 距离界面上的距离读数.
fn distance_value() {
    let text = display_text(30);
    let digits = "";
    for pos in 0..text.len() {
        let ch = text[pos];
        if ch >= '0' && ch <= '9' {
            digits += ch;
        }
    }
    if digits.len() == 0 { 0 } else { digits.parse_int() }
}

// 记录界面上的数值放大 10 倍后的整数.
fn record_value_x10() {
    let text = display_text(30);
    let digits = "";
    for pos in 0..text.len() {
        let ch = text[pos];
        if (ch >= '0' && ch <= '9') || ch == '.' {
            digits += ch;
        }
    }
    if digits.len() == 0 { 0 } else { (parse_float(digits) * 10.0).round().to_int() }
}

// 记录界面类型标识 (单个段码): 最大值用 a 段, 平均值用 g 段, 最小值用 d 段.
fn record_mark() {
    display_text(30)[1]
}

// 进入记录界面: 数据界面下按一次 S5 到距离界面, 再一次到记录界面.
fn goto_record() {
    press(S5);
    press(S5);
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与时间界面 (4.3-1, 4.7, 4.8)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与时间界面", "时间界面显示 20:20 后的时间, 触发模式与光亮指示, 参数默认 2s/20cm", || {
    boot(26, 4.0, 1);
    let time_text = display_text(30);
    let time_leds = led_text();

    press(S4);
    let p1 = display_text(30);
    press(S5);
    let p2 = display_text(30);

    assert_regex(time_text, "^20-20-\\d\\d$", "初始状态: 应处于时间数据显示界面");
    assert_eq(time_leds[0], '1', "指示灯: 时间界面下 L1 应点亮");
    assert_eq(time_leds[3], '1', "初始状态: 触发模式下 L4 应点亮");
    assert_eq(time_leds[5], '1', "指示灯: 环境光为亮时 L6 应点亮");
    assert_regex(p1, "^P1\\s+02$", "初始状态: 采集时间参数应为 2s");
    assert_regex(p2, "^P2\\s+20$", "初始状态: 距离参数应为 20cm");
    time_text + " " + time_leds + " " + p1 + " " + p2
});

// ---------------------------------------------------------------------------
// 检查点 2: 触发模式采集与距离显示 (4.3-2, 4.6-2, 4.7)
// ---------------------------------------------------------------------------
ckpt(2, "触发模式采集与距离显示", "环境光由亮变暗触发一次采集, 距离界面显示 L 与距离", || {
    boot(26, 4.0, 1);
    press(S5);
    let before = display_text(30);

    trigger_collect();
    wait_measure();
    let after = display_text(30);
    let value = distance_value();
    let leds = led_text();

    assert_regex(before, "^LC\\s+\\d+$", "距离界面: 应以 L 与模式标识 C 显示距离");
    assert_regex(after, "^LC\\s+\\d+$", "距离界面: 触发后仍应显示 L 与 C");
    assert_in(value, 22..=30, "测距精度: 26cm 的测距结果应不超过 ±4cm");
    assert_eq(leds[1], '1', "指示灯: 距离界面下 L2 应点亮");
    before + " -> " + after
});

// ---------------------------------------------------------------------------
// 检查点 3: 定时模式与 DAC 输出 (4.5, 4.6-1, 4.7)
// ---------------------------------------------------------------------------
ckpt(3, "定时模式与 DAC 输出", "S8 切到定时模式后按秒值整除采集时间采集, DAC 与距离成直线", || {
    boot(26, 4.0, 1);
    press(S5);
    press(S8);
    let mode_text = display_text(30);

    // 采集时间参数默认 2s, 等 4 秒覆盖两次整除触发.
    run_ms(4500);
    let value = distance_value();
    let mv = dac_mv();
    let leds = led_text();

    assert_regex(mode_text, "^LF\\s+\\d+$", "定时模式: 模式标识应变为 F");
    assert_in(value, 22..=30, "定时模式: 26cm 的测距结果应不超过 ±4cm");
    // 26cm 对应 1.0 + (26 - 10) / 70 * 4.0 = 1.91V.
    assert_in(mv, 1610..=2210, "DAC 输出: 26cm 附近应输出约 1.9V");
    assert_eq(leds[3], '0', "指示灯: 定时模式下 L4 应熄灭");
    mode_text + " " + value.to_string() + "cm " + mv.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 4: 数据记录界面 (4.3-3, 4.4)
// ---------------------------------------------------------------------------
ckpt(4, "数据记录界面", "记录最大/最小/平均值, 进入默认最大值, S8 按顺序切换且标识不同", || {
    boot(20, 4.0, 1);
    // 三次不同距离的采集: 20cm, 40cm, 30cm.
    for distance in [20, 40, 30] {
        set_distance_cm(distance);
        run_ms(1000);
        trigger_collect();
        wait_measure();
    }
    press(S5);
    press(S5);
    let max_page = display_text(30);
    let max_mark = record_mark();
    let max_value = record_value_x10();

    press(S8);
    let min_page = display_text(30);
    let min_mark = record_mark();
    let min_value = record_value_x10();

    press(S8);
    let avg_page = display_text(30);
    let avg_mark = record_mark();
    let avg_value = record_value_x10();
    let leds = led_text();

    assert_regex(max_page, "^H.\\s+\\d+$", "记录界面: 应以 H 与类型标识显示数据");
    assert_in(max_value, 380..=420, "记录数据: 最大值应约为 40cm");
    assert_in(min_value, 180..=220, "记录数据: 最小值应约为 20cm");
    assert_in(avg_value, 280..=320, "记录数据: 平均值应约为 30cm");
    // 题目用 a, g, d 段分别标识最大值, 平均值, 最小值, 解码表把这三个单段映射为 =, -, _.
    assert_eq(max_mark, '=', "记录界面: 最大值的类型标识应为 a 段");
    assert_eq(avg_mark, '-', "记录界面: 平均值的类型标识应为 g 段");
    assert_eq(min_mark, '_', "记录界面: 最小值的类型标识应为 d 段");
    assert_eq(leds[2], '1', "指示灯: 记录界面下 L3 应点亮");
    max_page + " | " + min_page + " | " + avg_page
});

// ---------------------------------------------------------------------------
// 检查点 5: 参数界面与生效 (4.3-4, 4.4)
// ---------------------------------------------------------------------------
ckpt(5, "参数界面与生效", "采集时间循环 2/3/5/7/9, 距离参数加 10 到 80 封顶, 退出后生效", || {
    boot(26, 4.0, 1);
    press(S4);
    let p1 = display_text(30);
    press(S9);
    let p1_next = display_text(30);
    press(S9);
    let p1_next2 = display_text(30);

    // 把采集时间循环回到 2s.
    for step in 0..3 {
        press(S9);
    }
    let p1_back = display_text(30);

    press(S5);
    let p2 = display_text(30);
    press(S9);
    let p2_up = display_text(30);
    // 连续加 6 次应始终落在 10-80 范围内 (题目只要求不越界, 未要求封顶).
    for step in 0..6 {
        press(S9);
    }
    let p2_max = display_text(30);
    // 跳过 "P2" 前缀后取数字.
    let tail = p2_max[3..];
    let digits = "";
    for pos in 0..tail.len() {
        let ch = tail[pos];
        if ch >= '0' && ch <= '9' {
            digits += ch;
        }
    }
    let p2_value = if digits.len() == 0 { 0 } else { digits.parse_int() };

    // 回到数据界面, 再进参数界面应默认采集时间界面.
    press(S4);
    press(S4);
    let default_page = display_text(30);

    assert_regex(p1, "^P1\\s+02$", "参数界面: 采集时间默认应为 02");
    assert_regex(p1_next, "^P1\\s+03$", "参数界面: 采集时间应按 2/3/5/7/9 循环");
    assert_regex(p1_next2, "^P1\\s+05$", "参数界面: 采集时间循环的第二个值应为 05");
    assert_regex(p1_back, "^P1\\s+02$", "参数界面: 采集时间应能循环回 02");
    assert_regex(p2, "^P2\\s+20$", "参数界面: 距离参数默认应为 20");
    assert_regex(p2_up, "^P2\\s+30$", "参数界面: 距离参数加 10 后应为 30");
    assert_regex(p2_max, "^P2\\s+\\d+$", "参数界面: 距离参数应保持两位数显示");
    assert_in(p2_value, 10..=80, "参数界面: 距离参数应始终落在 10-80 范围内");
    assert_regex(default_page, "^P1\\s+\\d\\d$", "界面切换: 每次进入参数界面应默认采集时间界面");
    p1 + " " + p1_next + " " + p2 + " " + p2_max
});

// ---------------------------------------------------------------------------
// 检查点 6: 距离报警与定时模式 (4.3-4, 4.7-5)
// ---------------------------------------------------------------------------
ckpt(6, "距离报警与定时模式", "定时模式下连续 3 次测距在距离参数 ±5cm 内时 L5 点亮", || {
    // 距离参数 20cm, 让测距结果稳定在 22cm 附近 (在 ±5cm 内).
    // 距离参数调到 40cm, 测距结果稳定在 39cm, 落在参数 ±5cm 的中心位置.
    boot(40, 4.0, 1);
    press(S4);
    press(S5);
    press(S9);
    press(S9);
    let param_page = display_text(30);
    press(S4);
    press(S5);
    press(S8);
    let mode_text = display_text(30);
    // 采集时间参数 2s, 等 8 秒可覆盖 4 次整除触发.
    run_ms(8000);
    let near_page = display_text(30);
    let near_value = distance_value();
    let near_leds = led_text();

    // 把距离改到 70cm (远离参数), 连续采集后 L5 应熄灭.
    set_distance_cm(70);
    run_ms(8000);
    let far_leds = led_text();

    assert_regex(param_page, "^P2\\s+40$", "距离报警: 距离参数应能调到 40cm");
    assert_regex(mode_text, "^LF\\s+\\d+$", "距离报警: 应先切到定时模式");
    assert_in(near_value, 35..=45, "距离报警: 测距结果应落在距离参数 ±5cm 内, actual=" + near_page);
    assert_eq(near_leds[4], '1', "距离报警: 连续 3 次测距在参数 ±5cm 内时 L5 应点亮, actual=" + near_page + " " + near_leds);
    assert_eq(far_leds[4], '0', "距离报警: 测距远离参数后 L5 应熄灭");
    near_leds + " -> " + far_leds
});

// ---------------------------------------------------------------------------
// 检查点 7: DAC 直线三点 (4.5)
// ---------------------------------------------------------------------------
ckpt(7, "DAC 直线三点", "10cm 输出 1.0V, 45cm 输出约 3.0V, 80cm 输出 5.0V", || {
    boot(10, 4.0, 1);
    press(S5);
    trigger_collect();
    wait_measure();
    let near = dac_mv();

    set_distance_cm(45);
    run_ms(1000);
    trigger_collect();
    wait_measure();
    let mid = dac_mv();

    set_distance_cm(80);
    run_ms(1000);
    trigger_collect();
    wait_measure();
    let far = dac_mv();

    assert_in(near, 700..=1300, "DAC 输出: 10cm 应输出 1.0V");
    assert_in(mid, 2700..=3300, "DAC 输出: 45cm 应输出约 3.0V");
    assert_in(far, 4700..=5300, "DAC 输出: 80cm 应输出 5.0V");
    near.to_string() + "mV " + mid.to_string() + "mV " + far.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 8: 响应时间要求 (4.2)
// ---------------------------------------------------------------------------
ckpt(8, "响应时间要求", "按键动作响应 <= 0.2s, 界面切换 <= 0.2s", || {
    boot(26, 4.0, 1);

    // 界面切换: 数据界面下按 S5 由时间页切到距离页, 应在 0.2 秒内完成.
    tap_key(S5, 60);
    let page_ms = 0;
    while page_ms < 1000 && display_text(10)[0..1] != "L" {
        run_ms(10);
        page_ms += 10;
    }
    let page_after = display_text(30);

    // 按键响应: 按 S4 由数据界面进入参数界面, 应在 0.2 秒内完成.
    tap_key(S4, 60);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..1] != "P" {
        run_ms(10);
        key_ms += 10;
    }
    let key_after = display_text(30);

    assert_in(page_ms, 0..=200, "响应时间: 界面切换应在 0.2 秒内完成, 实际 " + page_ms.to_string() + "ms, 页面 " + page_after);
    assert_in(key_ms, 0..=200, "响应时间: 按键响应应在 0.2 秒内完成, 实际 " + key_ms.to_string() + "ms, 页面 " + key_after);
    page_ms.to_string() + "ms, " + key_ms.to_string() + "ms"
});
