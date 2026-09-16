// 第十一届蓝桥杯单片机省赛 (第一次) 程序设计试题 (70 分) 整卷评测脚本.
//
// 题目原文是扫描件, 已用本地 OCR 转成文本 (见 problem/题目OCR文本.txt), 评测点按原文推导:
//   4.1 性能: 电压刷新 <= 0.5s, 电压采样 <= 0.1s, 界面切换 <= 0.3s;
//             参数存储占 E2PROM 内部地址 0 一个字节; 电压参数范围 0 <= VP <= 5.0.
//   4.2 显示: 数据界面 "U" + AIN3 电压 (2 位小数), 参数界面 "P" + 电压参数,
//             计数界面 "N" + 计数值.
//   4.3 按键: S12 循环切换 数据/参数/计数 三个界面; S13 在计数界面清零;
//             S16/S17 仅在参数界面把 VP 加减 0.5V 并在 5.00V 与 0.00V 之间回绕;
//             从参数界面退出时把 VP 放大 10 倍写入 E2PROM 内部地址 0.
//   4.3 计数条件: VAIN3 由不小于 VP 变到不大于 VP 时计数值加 1 (下降沿穿越电压参数).
//   4.4 LED: L1 在 VAIN3 < VP 持续超过 5 秒时点亮; L2 在计数值为奇数时点亮;
//             L3 在连续 3 次以上无效按键操作后点亮, 直到出现有效按键操作.
//   4.5 初始状态: 数据界面, 计数值 0, L2 熄灭, 上电从 E2PROM 内部地址 0 读出并处理为 VP.

key_mode(KBD);

fn wait_sample() {
    run_ms(800);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(250);
}

// 上电: E2PROM 地址 0 写入 eeprom_value (对应 VP = eeprom_value / 10).
fn boot(eeprom_value, ain3) {
    reset(POWER_RESET);
    key_mode(KBD);
    set_eeprom(0, eeprom_value);
    set_voltage(AIN3, ain3);
    run_ms(1500);
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

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

// 进入参数界面并把电压参数设为 target (0.00 起按 S16 逐次加 0.5V).
fn set_param(target) {
    press(S12);
    let steps = (target * 2.0).round().to_int();
    for step in 0..steps {
        press(S16);
    }
}

// 一次下降沿: VAIN3 先升到参数之上, 再降到参数之下.
fn down_edge(high, low) {
    set_voltage(AIN3, high);
    run_ms(600);
    set_voltage(AIN3, low);
    run_ms(600);
}

// 读取计数界面当前的计数值: 计数值为 0 时该位可能熄灭, 因此按整串显示文本判断.
fn count_value() {
    let text = display_text(30);
    let digits = "";
    for pos in 0..text.len() {
        let ch = text[pos];
        if ch >= '0' && ch <= '9' {
            digits += ch;
        }
    }
    if digits.len() == 0 {
        0
    } else {
        digits.parse_int()
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 数据界面与电压显示 (4.1, 4.2-1, 4.5)
// ---------------------------------------------------------------------------
ckpt(1, "数据界面与电压显示", "数据界面以 U 显示 AIN3 电压, 保留 2 位小数", || {
    boot(30, 3.24);
    let text = display_text(30);
    let value = (display_number(3, 8) * 100.0).round().to_int();

    set_voltage(AIN3, 1.10);
    wait_sample();
    let low = display_text(30);

    set_voltage(AIN3, 4.80);
    wait_sample();
    let high = display_text(30);

    assert_regex(text, "^U\\s+\\d\\.\\d\\d$", "数据界面: 应以 U 为提示符显示两位小数电压");
    assert_in(value, 319..=329, "数据界面: 3.24V 的读数超出容差");
    assert_regex(low, "^U\\s+1\\.\\d\\d$", "数据界面: 1.10V 的读数错误");
    assert_regex(high, "^U\\s+4\\.\\d\\d$", "数据界面: 4.80V 的读数错误");
    text + " " + low + " " + high
});

// ---------------------------------------------------------------------------
// 检查点 2: 参数界面与加减回绕 (4.2-2, 4.3)
// ---------------------------------------------------------------------------
ckpt(2, "参数界面与加减回绕", "S16/S17 步进 0.5V 并在 0.00V 与 5.00V 之间回绕", || {
    boot(0, 3.24);
    press(S12);
    let initial = display_text(30);

    press(S16);
    let up = display_text(30);

    // 从 0.50V 连续减两次: 0.50 -> 0.00 -> 5.00 (回绕).
    press(S17);
    let zero = display_text(30);
    press(S17);
    let wrap_high = display_text(30);

    // 从 5.00V 再加一次应回到 0.00V.
    press(S16);
    let wrap_low = display_text(30);

    assert_regex(initial, "^P\\s+\\d\\.\\d\\d$", "参数界面: 应以 P 为提示符显示两位小数电压参数");
    assert_regex(up, "^P\\s+0\\.50$", "参数界面: 加一次应到 0.50V");
    assert_regex(zero, "^P\\s+0\\.00$", "参数界面: 减到下限应为 0.00V");
    assert_regex(wrap_high, "^P\\s+5\\.00$", "参数界面: 低于 0.00V 后应回绕到 5.00V");
    assert_regex(wrap_low, "^P\\s+0\\.00$", "参数界面: 超过 5.00V 后应回绕到 0.00V");
    initial + " " + up + " " + zero + " " + wrap_high + " " + wrap_low
});

// ---------------------------------------------------------------------------
// 检查点 3: 计数界面与下降沿计数 (4.2-3, 4.3)
// ---------------------------------------------------------------------------
ckpt(3, "计数界面与下降沿计数", "电压从参数之上降到参数之下时计数值加 1, S13 在计数界面清零", || {
    boot(0, 4.50);
    // 参数设为 3.00V.
    set_param(3.0);
    press(S12);
    let count_page = display_text(30);
    // 上电初期的电压跳变可能已经产生计数, 先清零建立基准.
    press(S13);
    let baseline = count_value();

    down_edge(4.50, 2.00);
    let first = display_text(30);
    let first_count = count_value();
    down_edge(4.50, 2.00);
    let second = display_text(30);
    let second_count = count_value();

    // 只有上升沿 (由低到高) 不应计数.
    set_voltage(AIN3, 1.50);
    run_ms(600);
    set_voltage(AIN3, 4.50);
    run_ms(600);
    let after_rise = display_text(30);
    let rise_count = count_value();

    press(S13);
    let cleared = display_text(30);
    let cleared_count = count_value();

    assert_regex(count_page, "^[Nn]\\s+\\d+$", "计数界面: 应以 N 为提示符显示计数值 (七段数码管一般用小写 n 近似大写 N)");
    assert_eq(baseline, 0, "计数: S13 清零后计数值应为 0");
    assert_eq(first_count, 1, "计数: 一次下降沿后计数值应为 1");
    assert_eq(second_count, 2, "计数: 两次下降沿后计数值应为 2");
    assert_eq(rise_count, 2, "计数: 只有上升沿时计数值不应变化");
    assert_eq(cleared_count, 0, "计数: S13 应把计数值清零");
    count_page + " -> " + first + " -> " + second + " -> " + cleared
});

// ---------------------------------------------------------------------------
// 检查点 4: E2PROM 参数存储与上电读取 (4.1, 4.3, 4.5-2)
// ---------------------------------------------------------------------------
ckpt(4, "E2PROM 参数存储与上电读取", "退出参数界面时存 VP*10 到地址 0, 上电时读出并作为 VP", || {
    // 参数设为 2.50V 后退出参数界面, E2PROM 地址 0 应写入 25.
    boot(0, 3.24);
    set_param(2.5);
    press(S12);
    let stored = eeprom_byte(0);

    // 重新上电, 地址 0 中已有 25, 参数界面应显示 2.50V.
    reset(POWER_RESET);
    key_mode(KBD);
    run_ms(1500);
    press(S12);
    let loaded = display_text(30);

    assert_eq(stored, 25, "E2PROM: 退出参数界面时应把 2.50V 放大 10 倍写入地址 0");
    assert_regex(loaded, "^P\\s+2\\.50$", "E2PROM: 上电应从地址 0 读回参数并显示为 2.50V");
    stored.to_string() + " -> " + loaded
});

// ---------------------------------------------------------------------------
// 检查点 5: L1 超时提醒 (4.4-1)
// ---------------------------------------------------------------------------
ckpt(5, "L1 超时提醒", "VAIN3 小于电压参数持续超过 5 秒时 L1 点亮", || {
    boot(0, 1.00);
    set_param(4.0);
    press(S12);
    press(S12);
    let before = led_text();

    // 保持 VAIN3 小于参数 6 秒.
    run_ms(6000);
    let after = led_text();

    // 把电压抬到参数之上, L1 应熄灭.
    set_voltage(AIN3, 4.80);
    run_ms(1500);
    let recovered = led_text();

    assert_eq(before[0], '0', "L1: 未达到 5 秒超时前应熄灭");
    assert_eq(after[0], '1', "L1: VAIN3 小于参数持续超过 5 秒应点亮");
    assert_eq(recovered[0], '0', "L1: 电压回到参数之上后应熄灭");
    before + " -> " + after + " -> " + recovered
});

// ---------------------------------------------------------------------------
// 检查点 6: L2 与 L3 指示 (4.4-2, 4.4-3)
// ---------------------------------------------------------------------------
ckpt(6, "L2 与 L3 指示", "L2 计数值为奇数时亮, L3 连续 3 次无效按键后亮并在有效按键后熄灭", || {
    boot(0, 4.50);
    set_param(3.0);
    press(S12);
    // 先清零, 再做一次下降沿得到奇数计数值.
    press(S13);
    down_edge(4.50, 2.00);
    let odd = led_text();

    press(S13);
    let even = led_text();

    // 数据界面下按 S16/S17 是无效按键: 连续 3 次后 L3 应点亮.
    press(S12);
    press(S16);
    let after_one = led_text();
    press(S17);
    press(S16);
    let after_three = led_text();

    // 在参数界面按 S16 是有效按键, L3 应熄灭.
    press(S12);
    press(S16);
    let valid = led_text();

    assert_eq(odd[1], '1', "L2: 计数值为 1 时 L2 应点亮");
    assert_eq(even[1], '0', "L2: 计数值清零后 L2 应熄灭");
    assert_eq(after_one[2], '0', "L3: 一次无效按键后不应点亮");
    assert_eq(after_three[2], '1', "L3: 连续 3 次无效按键后应点亮");
    assert_eq(valid[2], '0', "L3: 出现有效按键操作后应熄灭");
    odd + " -> " + even + " -> " + after_three + " -> " + valid
});
