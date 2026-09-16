// 第十一届蓝桥杯单片机省赛 (第二次) 程序设计试题 (70 分) 整卷评测脚本.
//
// 题目原文是扫描件, 已用本地 OCR 转成文本 (见 problem/题目OCR文本.txt), 评测点按原文推导:
//   4.2 性能: 温度刷新 <= 0.5s, 界面切换 <= 0.3s, DAC 输出响应 <= 0.3s;
//             温度上限与下限参数范围 0 <= T < 100.
//   4.3 显示: 数据界面 "C" + 温度整数; 参数界面 "P" + 温度上限参数 + 温度下限参数.
//   4.4 按键: S4 切换数据界面与参数界面, S5 切换选中的温度参数, S6/S7 加减 1 摄氏度,
//             加减功能仅在参数界面有效; 每次从数据界面进入参数界面默认选中温度下限参数;
//             从参数界面退出时做合理性检查 (上限 >= 下限), 合理则生效,
//             否则恢复进入参数界面前的有效参数并点亮 L4.
//   4.5 DAC: 温度高于上限输出 4.0V, 处于上下限之间输出 3.0V, 低于下限输出 2.0V.
//   4.6 LED: L1 温度高于上限, L2 温度处于上下限之间, L3 温度低于下限,
//             L4 表示出现过错误参数设置, 直到下一次正确设置后熄灭.
//   4.7 初始状态: 数据界面, 温度上限参数 30, 温度下限参数 20.

key_mode(BTN);

fn wait_measure() {
    // DS18B20 一次转换与刷新需要约 1 秒, 这里留出两倍余量 (题目要求刷新 <= 0.5s).
    run_ms(2000);
}

// 轮询等待条件成立, 返回实际耗时 (毫秒); 到上限仍未成立时返回上限.
// 相比 run_to, 这样即使条件始终不成立也能拿到可记录的实际耗时.
fn wait_until(predicate, limit_ms, step_ms) {
    let elapsed = 0;
    while elapsed < limit_ms {
        if predicate() {
            return elapsed;
        }
        run_ms(step_ms);
        elapsed += step_ms;
    }
    elapsed
}

// 题目 4.2 的响应时间要求, 用于性能断言.
fn switch_ns() {
    300_000_000
}

fn temp_refresh_ns() {
    500_000_000
}

fn dac_response_ns() {
    300_000_000
}

fn press(key) {
    tap_key(key, 60);
    run_ms(250);
}

fn boot(temp) {
    reset(POWER_RESET);
    key_mode(BTN);
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

// 参数界面当前显示的两个参数值 (左为上限, 右为下限).
fn param_pair() {
    let text = display_text(30);
    let digits = "";
    for pos in 0..text.len() {
        let ch = text[pos];
        if ch >= '0' && ch <= '9' {
            digits += ch;
        } else {
            digits += " ";
        }
    }
    let parts = digits.split(" ");
    let values = [];
    for part in parts {
        if part.len() > 0 {
            values.push(part.parse_int());
        }
    }
    values
}

// 从数据界面进入参数界面并把选中的参数设为 target (S6 逐次加 1, 从当前值出发).
fn set_selected(current, target) {
    let steps = target - current;
    let count = if steps >= 0 { steps } else { -steps };
    for step in 0..count {
        if steps >= 0 {
            press(S6);
        } else {
            press(S7);
        }
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与数据界面 (4.3-1, 4.7)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与数据界面", "数据界面显示整数温度, 默认参数 30/20, 温度处于区间内", || {
    boot(24.0);
    let data_text = display_text(30);
    let data_leds = led_text();
    let data_dac = dac_mv();

    press(S4);
    let param_text = display_text(30);
    let pair = param_pair();
    press(S4);

    assert_regex(data_text, "^C\\s+\\d+$", "数据界面: 应以 C 为提示符显示整数温度");
    assert_eq(display_number(2, 8), 24, "数据界面: 24 摄氏度的读数应为 24");
    assert_regex(param_text, "^P\\s+30\\s+20$", "初始状态: 参数界面应显示上限 30 与下限 20");
    assert_eq(pair.len(), 2, "参数界面: 应能读出上限与下限两个参数");
    assert_eq(data_leds[1], '1', "LED: 温度处于上下限之间时 L2 应点亮");
    assert_in(data_dac, 2900..=3100, "DAC: 温度处于上下限之间应输出 3.0V");
    data_text + ", " + param_text + ", " + data_dac.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 2: 参数界面加减与默认选中项 (4.4)
// ---------------------------------------------------------------------------
ckpt(2, "参数界面加减与默认选中项", "每次进入参数界面默认选中温度下限, S6/S7 只影响选中的参数", || {
    boot(24.0);
    press(S4);
    let initial = param_pair();

    // 默认应选中温度下限参数 20: 加一次后下限变 21, 上限仍为 30.
    press(S6);
    let lower_up = param_pair();

    // 切到另一个参数再加一次, 此时上限应变 31, 下限保持 21.
    press(S5);
    press(S6);
    let upper_up = param_pair();

    // 数据界面下按 S6/S7 不应改变参数.
    press(S4);
    press(S6);
    press(S7);
    press(S4);
    let unchanged = param_pair();

    assert_eq(initial[0], 30, "初始状态: 温度上限参数应为 30");
    assert_eq(initial[1], 20, "初始状态: 温度下限参数应为 20");
    assert_eq(lower_up[0], 30, "参数界面: 默认选中温度下限时上限不应变化");
    assert_eq(lower_up[1], 21, "参数界面: 默认选中温度下限, 加一次后下限应为 21");
    assert_eq(upper_up[0], 31, "参数界面: 切换后加一次应使上限变为 31");
    assert_eq(upper_up[1], 21, "参数界面: 未选中的下限应保持 21");
    assert_eq(unchanged[0], 31, "按键范围: 数据界面下 S6 不应改变上限");
    assert_eq(unchanged[1], 21, "按键范围: 数据界面下 S7 不应改变下限");
    initial.to_string() + " -> " + lower_up.to_string() + " -> " + upper_up.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 3: 参数合理性检查与恢复 (4.4-2)
// ---------------------------------------------------------------------------
ckpt(3, "参数合理性检查与恢复", "退出参数界面时上限小于下限则恢复原参数并点亮 L4", || {
    boot(24.0);
    press(S4);
    // 把下限从 20 调到 35, 使其超过上限 30.
    set_selected(20, 35);
    let invalid_pair = param_pair();
    press(S4);
    // 退出参数界面后要重新进入参数界面才能读到生效的参数.
    press(S4);
    let after_invalid = param_pair();
    let error_leds = led_text();
    press(S4);

    // 再次进入参数界面: 错误参数已被恢复为 20, 把它调到 25 再退出, L4 应熄灭.
    press(S4);
    set_selected(20, 25);
    press(S4);
    press(S4);
    let after_valid = param_pair();
    let recovered_leds = led_text();
    press(S4);

    assert_eq(invalid_pair[1], 35, "参数界面: 应能把下限调到 35");
    assert_eq(after_invalid[0], 30, "参数检查: 上限小于下限时参数应恢复为 30");
    assert_eq(after_invalid[1], 20, "参数检查: 上限小于下限时参数应恢复为 20");
    assert_eq(error_leds[3], '1', "参数检查: 出现错误参数设置后 L4 应点亮");
    assert_eq(after_valid[1], 25, "参数检查: 正确设置下限 25 后应生效");
    assert_eq(recovered_leds[3], '0', "参数检查: 下一次正确设置后 L4 应熄灭");
    invalid_pair.to_string() + " -> " + after_invalid.to_string() + " -> " + after_valid.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 4: DAC 三档输出 (4.2-3, 4.5)
// ---------------------------------------------------------------------------
ckpt(4, "DAC 三档输出", "温度高于上限输出 4.0V, 区间内输出 3.0V, 低于下限输出 2.0V", || {
    boot(35.0);
    let hot = dac_mv();

    set_temperature_c(25.0);
    wait_measure();
    let mid = dac_mv();

    set_temperature_c(15.0);
    wait_measure();
    let cold = dac_mv();

    assert_in(hot, 3900..=4100, "DAC: 温度 35 摄氏度高于上限 30 应输出 4.0V");
    assert_in(mid, 2900..=3100, "DAC: 温度 25 摄氏度处于 20-30 之间应输出 3.0V");
    assert_in(cold, 1900..=2100, "DAC: 温度 15 摄氏度低于下限 20 应输出 2.0V");
    hot.to_string() + "mV " + mid.to_string() + "mV " + cold.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 5: LED 区间指示 (4.6-1, 4.6-2, 4.6-3)
// ---------------------------------------------------------------------------
ckpt(5, "LED 区间指示", "L1/L2/L3 分别对应高于上限, 区间内, 低于下限", || {
    boot(35.0);
    let hot = led_text();

    set_temperature_c(25.0);
    wait_measure();
    let mid = led_text();

    set_temperature_c(15.0);
    wait_measure();
    let cold = led_text();

    assert_eq(hot[0..3], "100", "LED: 温度高于上限时应只点亮 L1");
    assert_eq(mid[0..3], "010", "LED: 温度处于区间内时应只点亮 L2");
    assert_eq(cold[0..3], "001", "LED: 温度低于下限时应只点亮 L3");
    assert_eq(mid[3..8], "00000", "LED: 其余指示灯应熄灭");
    hot[0..3] + " -> " + mid[0..3] + " -> " + cold[0..3]
});

// ---------------------------------------------------------------------------
// 检查点 6: 参数边界与上下限生效 (4.2-4, 4.4-2)
// ---------------------------------------------------------------------------
ckpt(6, "参数边界与生效", "参数在 0-99 范围内, 新参数退出参数界面后生效", || {
    // 下限先减到 0 再减两次 (考察边界), 然后加回 20 使参数合法.
    boot(24.0);
    press(S4);
    set_selected(20, 0);
    press(S7);
    press(S7);
    let at_zero = param_pair();

    // 下限加回 20, 再把上限从 30 减到 25, 退出后温度 24 应处于区间内.
    set_selected(at_zero[1], 20);
    press(S5);
    set_selected(30, 25);
    let adjusted = param_pair();
    press(S4);
    wait_measure();
    let mid_dac = dac_mv();

    // 温度 30 度高于新上限 25: 应输出 4.0V.
    set_temperature_c(30.0);
    wait_measure();
    let hot_dac = dac_mv();

    // 题目只要求参数不越界 (0 <= T < 100), 因此这里按范围判定, 不强制夹紧在 0.
    assert_in(at_zero[1], 0..=99, "参数边界: 下限减到 0 后继续减, 参数应保持在 0-99 范围内");
    assert_eq(adjusted[0], 25, "参数: 上限应能调到 25");
    assert_in(mid_dac, 2900..=3100, "参数生效: 温度 24 处于 0-25 之间应输出 3.0V");
    assert_in(hot_dac, 3900..=4100, "参数生效: 温度 30 高于新上限 25 应输出 4.0V");
    at_zero.to_string() + " -> " + adjusted.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 7: 响应时间要求 (4.2)
// ---------------------------------------------------------------------------
ckpt(7, "显示与输出响应时间", "界面切换 <= 0.3s, 温度刷新 <= 0.5s, DAC 响应 <= 0.3s", || {
    boot(24.0);
    // 界面切换: 按下 S4 后应很快进入参数界面 (轮询到上限为止, 记录实际耗时).
    tap_key(S4, 60);
    let switch_ms = 0;
    while switch_ms < 2000 && display_text(10)[0..1] != "P" {
        run_ms(20);
        switch_ms += 20;
    }
    press(S4);

    // 温度刷新: 环境温度改到 35 摄氏度后, 数码管应在题目要求的时间内更新.
    set_temperature_c(35.0);
    let temp_ms = 0;
    while temp_ms < 3000 && display_number(2, 8) != 35 {
        run_ms(50);
        temp_ms += 50;
    }
    let temp_page = display_text(30);

    // DAC 响应: 温度升到 35 摄氏度 (高于上限 30, 应输出 4.0V) 后, DAC 应在 0.3 秒内跟上.
    let dac_ms = 0;
    while dac_ms < 3000 && da_value() < 200 {
        run_ms(20);
        dac_ms += 20;
    }

    assert(switch_ms <= 300, "响应时间: 界面切换应在 0.3 秒内完成, 实际 " + switch_ms.to_string() + "ms");
    assert(temp_ms <= 500, "响应时间: 温度刷新应在 0.5 秒内完成, 实际 " + temp_ms.to_string() + "ms, 页面 " + temp_page);
    assert(dac_ms <= 300, "响应时间: DAC 输出应在 0.3 秒内跟上温度变化, 实际 " + dac_ms.to_string() + "ms");
    switch_ms.to_string() + "ms, " + temp_ms.to_string() + "ms, " + dac_ms.to_string() + "ms"
});
