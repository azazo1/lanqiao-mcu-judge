// 第十二届蓝桥杯单片机省赛第一场 程序设计试题 (70 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.2 性能: 温度刷新 <= 1s, DAC 刷新 <= 0.5s, 按键响应 <= 0.2s.
//   4.3 显示: 温度界面 "C" + 两位小数温度, 参数界面 "P" + 整数参数, DAC 界面 "A" + 两位小数电压.
//   4.4 按键: S4 循环切换三个界面, S8/S9 只在参数界面改 1 度, S5 切换模式 1/2.
//   4.4 模式 1: 实时温度 < 参数 输出 0V, 否则输出 5V.
//   4.4 模式 2: 20 度输出 1.0V, 40 度输出 4.0V, 中间线性 (图 7).
//   4.5 LED: L1 表示模式 1, L2/L3/L4 分别表示温度页/参数页/DAC 页.
//   4.6 上电初始状态: 温度界面, 模式 1, 参数 25 度.
//
// 参数在离开参数界面时生效 (4.4), 因此涉及参数的评测点都按 "先退出再读输出" 的顺序组织.

key_mode(KEYBOARD);

// 题目要求 J5 为 KBD 模式, J13 为 IO 模式, 这两项是板级跳线约定.
//
// 时间窗口按题目性能要求取值: 温度刷新 1s, DAC 刷新 0.5s, 按键响应 0.2s.
// Rhai 的顶层常量对函数不可见, 因此各窗口在用到的地方就地声明.

fn dac_mv() {
    // DAC 为 8 位输出, 满量程按 5V 换算.
    const DAC_FULL_SCALE_MV = 5000;
    da_value() * DAC_FULL_SCALE_MV / 255
}

// 等待一个 DAC 刷新窗口 (题目要求 <= 0.5s).
fn wait_dac() {
    run_ms(500);
}

// 等待一个按键响应窗口 (题目要求 <= 0.2s).
fn wait_key_response() {
    run_ms(200);
}

// 把温度值换算成 "厘摄氏度" 整数, 便于用整数区间做容差断言.
fn temp_cents(value) {
    (value * 100.0).round().to_int()
}

// 上电并等待温度刷新窗口: 题目允许温度 1s 内刷新.
fn boot(temp) {
    const TEMP_REFRESH_MS = 1000;
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    set_temperature_c(temp);
    run_ms(TEMP_REFRESH_MS + 100);
}

// 改变环境温度后等待一个完整的温度刷新窗口.
fn set_ambient(temp) {
    const TEMP_REFRESH_MS = 1000;
    set_temperature_c(temp);
    run_ms(TEMP_REFRESH_MS);
}

// 按键并按题目给出的响应时间上限等待界面稳定.
fn press(key) {
    const KEY_RESPONSE_MS = 200;
    tap_key(key, 60);
    wait_key_response();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态 (4.6)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态", "温度界面, 模式 1, 参数 25 度, L1/L2 点亮", || {
    boot(25.0);
    let text = display_text(30);
    let leds = led_text();

    assert_regex(text, "^C\\s*25\\.00$", "上电初始状态: 温度界面显示应为 C 与 25.00");
    assert_eq(leds[0], '1', "上电初始状态: 模式 1 时 L1 应点亮");
    assert_eq(leds[1], '1', "上电初始状态: 温度界面时 L2 应点亮");
    assert_eq(leds[2], '0', "上电初始状态: 参数界面未选中时 L3 应熄灭");
    assert_eq(leds[3], '0', "上电初始状态: DAC 界面未选中时 L4 应熄灭");
    text
});

// ---------------------------------------------------------------------------
// 检查点 2: 温度显示与刷新时间 (4.2-1, 4.3-1)
// ---------------------------------------------------------------------------
ckpt(2, "温度显示 18.75 度", "C 界面显示两位小数温度, 1s 内刷新", || {
    boot(25.0);
    set_ambient(18.75);
    let text = display_text(30);
    let cents = temp_cents(display_number(5, 8));

    assert_regex(text, "^C\\s*18\\.75$", "温度界面: 显示应为 C 与 18.75");
    assert_in(cents, 1870..=1880, "温度界面: 温度数值超出容差");
    text
});

// ---------------------------------------------------------------------------
// 检查点 3: S4 界面循环与界面 LED 指示 (4.3, 4.4, 4.5)
// ---------------------------------------------------------------------------
ckpt(3, "S4 循环切换三个界面", "温度 -> 参数 -> DAC -> 温度, 界面 LED 同步", || {
    boot(23.5);

    let temp_text = display_text(30);
    let temp_leds = led_text();

    press(S4);
    let param_text = display_text(30);
    let param_leds = led_text();

    press(S4);
    let dac_text = display_text(30);
    let dac_leds = led_text();

    press(S4);
    let back_text = display_text(30);
    let back_leds = led_text();

    assert_eq(temp_text[0], 'C', "温度界面: 标识符应为 C");
    assert_eq(param_text[0], 'P', "参数界面: 标识符应为 P");
    assert_eq(dac_text[0], 'A', "DAC 界面: 标识符应为 A");
    assert_eq(back_text[0], 'C', "S4 第三次切换: 应回到温度界面");

    assert_eq(temp_leds, "11000000", "温度界面: 应为 L1+L2 点亮");
    assert_eq(param_leds, "10100000", "参数界面: 应为 L1+L3 点亮");
    assert_eq(dac_leds, "10010000", "DAC 界面: 应为 L1+L4 点亮");
    assert_eq(back_leds, "11000000", "回到温度界面: 应为 L1+L2 点亮");

    // 三次单次按键后仍停在温度页, 说明一次按键只触发一次切换 (消抖正确).
    back_text
});

// ---------------------------------------------------------------------------
// 检查点 4: 参数界面 S8/S9 加减与作用范围 (4.4)
// ---------------------------------------------------------------------------
ckpt(4, "参数界面 S8/S9 调整参数", "S9 加 1, S8 减 1, 其他界面无效", || {
    boot(23.5);

    press(S4);
    let initial = display_text(30);

    press(S9);
    press(S9);
    let after_up = display_text(30);

    press(S8);
    let after_down = display_text(30);

    // 在温度界面按 S8/S9 不应改变参数.
    press(S4);
    press(S4);
    let temp_page = display_text(30);
    press(S8);
    press(S9);
    let temp_page_after = display_text(30);

    press(S4);
    let param_again = display_text(30);

    assert_regex(initial, "^P\\s*25$", "参数界面: 上电参数应为 25");
    assert_regex(after_up, "^P\\s*27$", "参数界面: S9 两次后参数应为 27");
    assert_regex(after_down, "^P\\s*26$", "参数界面: S8 一次后参数应为 26");
    assert_eq(temp_page[0], 'C', "S8/S9 作用范围: 应已回到温度界面");
    assert_eq(temp_page_after, temp_page, "S8/S9 作用范围: 温度界面显示不应被改变");
    assert_regex(param_again, "^P\\s*26$", "S8/S9 作用范围: 温度界面按键不得改动参数");
    after_down
});

// ---------------------------------------------------------------------------
// 检查点 5: 模式 1 的 DAC 输出 (4.4)
// ---------------------------------------------------------------------------
ckpt(5, "模式 1 DAC 输出", "温度低于参数输出 0V, 参数下调后输出 5V", || {
    boot(24.25);

    // 24.25 < 25, 模式 1 应输出 0V.
    wait_dac();
    let low_mv = dac_mv();

    // 参数下调到 24 并退出参数界面后生效: 24.25 >= 24, 应输出 5V.
    press(S4);
    press(S8);
    let param_text = display_text(30);
    press(S4);
    press(S4);
    wait_dac();
    let high_mv = dac_mv();

    assert_in(low_mv, 0..=60, "模式 1: 温度低于参数时 DAC 应输出 0V");
    assert_regex(param_text, "^P\\s*24$", "模式 1: 参数应被 S8 下调到 24");
    assert_in(high_mv, 4940..=5000, "模式 1: 温度不低于参数时 DAC 应输出 5V");
    low_mv.to_string() + "mV -> " + high_mv.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 6: 模式 2 的线性 DAC 输出 (4.4, 图 7)
// ---------------------------------------------------------------------------
ckpt(6, "模式 2 线性 DAC 输出", "20 度 1.0V, 40 度 4.0V, 30 度 2.5V", || {
    boot(20.0);

    // S5 切换到模式 2, L1 应熄灭.
    press(S5);
    let mode2_leds = led_text();

    set_ambient(20.0);
    wait_dac();
    let at_20 = dac_mv();

    set_ambient(40.0);
    wait_dac();
    let at_40 = dac_mv();

    set_ambient(30.0);
    wait_dac();
    let at_30 = dac_mv();

    // 再按一次 S5 回到模式 1, L1 应重新点亮.
    press(S5);
    let mode1_leds = led_text();

    assert_eq(mode2_leds[0], '0', "模式切换: 模式 2 时 L1 应熄灭");
    assert_eq(mode1_leds[0], '1', "模式切换: 回到模式 1 时 L1 应点亮");
    assert_in(at_20, 950..=1050, "模式 2: 20 度应输出 1.0V");
    assert_in(at_40, 3950..=4050, "模式 2: 40 度应输出 4.0V");
    assert_in(at_30, 2450..=2550, "模式 2: 30 度应输出 2.5V");
    at_20.to_string() + "mV," + at_30.to_string() + "mV," + at_40.to_string() + "mV"
});

// ---------------------------------------------------------------------------
// 检查点 7: DAC 界面电压显示与刷新时间 (4.2-2, 4.3-3)
// ---------------------------------------------------------------------------
ckpt(7, "DAC 界面电压显示", "A 界面显示两位小数电压并 0.5s 内刷新", || {
    boot(20.0);

    press(S5);
    set_ambient(40.0);
    // 进入 DAC 界面: 温度页 -> 参数页 -> DAC 页.
    press(S4);
    press(S4);
    wait_dac();
    let text_40 = display_text(30);

    // 温度刷新 1s 后 DAC 应在 0.5s 内跟上, 20 度对应 1.0V.
    set_ambient(20.0);
    wait_dac();
    let text_20 = display_text(30);

    assert_regex(text_40, "^A\\s*4\\.00$", "DAC 界面: 40 度时应显示 A 与 4.00");
    assert_regex(text_20, "^A\\s*1\\.00$", "DAC 界面: 20 度时应显示 A 与 1.00");
    text_40 + " -> " + text_20
});

// ---------------------------------------------------------------------------
// 检查点 8: 按键响应时间 (4.2-3)
// ---------------------------------------------------------------------------
ckpt(8, "按键响应时间", "按下 S4 后 200ms 内完成界面切换", || {
    boot(23.5);

    // tap_key 已包含按下与释放, 这里只再等题目允许的 200ms 响应窗口.
    tap_key(S4, 60);
    wait_key_response();
    let text = display_text(0);

    assert_eq(text[0], 'P', "按键响应: 200ms 内应由温度界面切到参数界面");
    text
});

// ---------------------------------------------------------------------------
// 检查点 9: 数据刷新时间 (性能要求)
// ---------------------------------------------------------------------------
ckpt(9, "数据刷新时间", "温度数据刷新 <= 1s, DAC 输出电压刷新 <= 0.5s", || {
    boot(20.0);
    // 切到模式 2, 温度变化会直接带动 DAC 输出.
    press(S5);

    // 温度刷新: 环境温度改到 35 度后, 温度界面应在 1 秒内更新.
    set_ambient(35.0);
    let temp_ms = 0;
    while temp_ms < 3000 && (display_number(2, 8) * 100.0 - 3500.0).abs() > 5.0 {
        run_ms(20);
        temp_ms += 20;
    }
    let temp_page = display_text(30);

    // DAC 刷新: 温度数据刷新后, 输出应在一个 0.5 秒内跟上 (35 度对应 3.25V).
    let dac_ms = 0;
    while dac_ms < 2000 && (dac_mv() - 3250).abs() > 150 {
        run_ms(10);
        dac_ms += 10;
    }
    let mv = dac_mv();

    assert_in(temp_ms, 0..=1000, "刷新时间: 温度数据应在 1 秒内刷新, 实际 " + temp_ms.to_string() + "ms, 页面 " + temp_page);
    assert_in(dac_ms, 0..=500, "刷新时间: DAC 输出电压应在 0.5 秒内刷新, 实际 " + dac_ms.to_string() + "ms, 输出 " + mv.to_string() + "mV");
    temp_ms.to_string() + "ms, " + dac_ms.to_string() + "ms"
});
