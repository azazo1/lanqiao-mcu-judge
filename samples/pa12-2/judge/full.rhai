// 第十二届蓝桥杯单片机省赛第二场 (第二批) 程序设计试题 (70 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导, 不参考样例实现的输出:
//   4.1 硬件: NE555 输出经跳线接到 P34 测频, PCF8591 采集 AIN1 (光敏 RD1) 与 AIN3 (电位器 Rb2).
//   4.2 性能: 数据刷新 <= 1s, 频率精度 <= ±8%, 电压精度 <= ±5%, 按键响应 <= 0.2s.
//   4.3 显示: 频率界面 "F" + 频率 (7 位数码管, 高位熄灭),
//             周期界面 "N" + 周期 (微秒), 电压界面 "U" + 通道号 + 两位小数电压.
//   4.4 按键: S4 循环切换频率/周期/电压界面, 电压界面用 S5 切通道 1/3,
//             S6 缓存通道 3 电压, S7 缓存频率, 长按 S7 超过 1s 松开切换 LED 功能.
//   4.5 LED: L1 通道 3 实时电压高于缓存电压, L2 实时频率高于缓存频率,
//            L3/L4/L5 分别表示频率/周期/电压界面, L6-L8 熄灭.
//   4.6 上电初始状态: 频率界面, LED 指示灯功能开启.
//
// 频率是窗口测量, 输入频率突变后的第一次测量会跨越新旧两个窗口, 因此改频后要等一个完整的
// 测量窗口再读显示; 数据刷新时间本身仍按题目要求的 1s 检查.

key_mode(BTN);

// 频率测量窗口与数据刷新时间的等待量: 题目要求数据 1s 内刷新, 这里再留一个窗口观察稳定值.
fn wait_measure() {
    run_ms(1800);
}

// 等待一个按键响应窗口 (题目要求 <= 0.2s).
fn wait_key_response() {
    run_ms(200);
}

fn press(key) {
    tap_key(key, 60);
    wait_key_response();
}

// 长按键: 按住超过 1s 后松开, 用于 S7 的 LED 功能开关.
fn long_press(key) {
    set_key(key, true);
    run_ms(1200);
    set_key(key, false);
    wait_key_response();
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
}

// 电压显示保留两位小数, 用 "厘伏" 整数做容差比较.
fn volt_cents(value) {
    (value * 100.0).round().to_int()
}

// 上电并等待一次完整测量: freq 为 NE555 输出频率, 两路 ADC 电压单位为伏.
fn boot(freq, ain1, ain3) {
    reset(POWER_RESET);
    key_mode(BTN);
    jumper_on(NET_SIG, SIG_OUT);
    set_frequency_hz(freq);
    set_voltage(AIN1, ain1);
    set_voltage(AIN3, ain3);
    wait_measure();
}

// ---------------------------------------------------------------------------
// 检查点 1: 上电初始状态与频率显示 (4.3-1, 4.5, 4.6)
// ---------------------------------------------------------------------------
ckpt(1, "上电初始状态与频率界面", "频率界面, L3 点亮, 显示 2035Hz", || {
    boot(2035, 3.25, 1.80);
    let text = display_text(30);
    let leds = led_text();
    let freq = display_number(2, 8);

    assert_eq(text[0], 'F', "初始状态: 应处于频率界面");
    assert_regex(text, "^F\\s+2035$", "频率界面: 应显示 F 与 2035, 高位熄灭");
    assert_in(freq, 1872..=2198, "频率界面: 频率值超出 ±8% 精度要求");
    assert_eq(leds, "00100000", "初始状态: 应为 L3 点亮, 其余指示灯熄灭");
    text
});

// ---------------------------------------------------------------------------
// 检查点 2: 频率界面刷新与高位数码管熄灭 (4.2-1, 4.3-1)
// ---------------------------------------------------------------------------
ckpt(2, "频率界面数据刷新", "改频后 1s 内刷新, 频率位数不足 7 位时高位熄灭", || {
    boot(1000, 3.25, 1.80);
    let text_1000 = display_text(30);

    set_frequency_hz(12345);
    run_ms(1000);
    let refreshed = display_text(0);
    wait_measure();
    let text_12345 = display_text(30);
    let freq = display_number(2, 8);

    assert_regex(text_1000, "^F\\s+1000$", "频率界面: 1000Hz 显示错误");
    // 题目要求数据刷新时间 <= 1s, 新频率在 1s 内至少要反映到显示上.
    assert(refreshed != text_1000, "频率界面: 改频后 1s 内显示未刷新, actual=" + refreshed);
    assert_regex(text_12345, "^F\\s+12345$", "频率界面: 12345Hz 显示错误, 高位应熄灭");
    assert_in(freq, 11357..=13332, "频率界面: 频率值超出 ±8% 精度要求");
    text_1000 + " -> " + text_12345
});

// ---------------------------------------------------------------------------
// 检查点 3: 周期界面 (4.3-2)
// ---------------------------------------------------------------------------
ckpt(3, "周期界面", "S4 切到周期界面, 显示 1000Hz 对应 1000μs", || {
    boot(1000, 3.25, 1.80);
    press(S4);
    let text = display_text(30);
    let leds = led_text();
    let period = display_number(2, 8);

    assert_eq(text[0], 'N', "周期界面: 标识符应为 N");
    assert_in(period, 920..=1080, "周期界面: 周期值超出 ±8% 精度要求");
    assert_eq(leds, "00010000", "周期界面: 应为 L4 点亮, 其余指示灯熄灭");
    text
});

// ---------------------------------------------------------------------------
// 检查点 4: 电压界面与 S5 通道切换 (4.3-3, 4.4-1)
// ---------------------------------------------------------------------------
ckpt(4, "电压界面与通道切换", "先显示通道 1, S5 切到通道 3, 精度 ±5%", || {
    boot(1000, 3.25, 1.80);
    press(S4);
    press(S4);
    let channel1 = display_text(30);
    let channel1_leds = led_text();
    let channel1_mv = volt_cents(display_number(6, 8));

    press(S5);
    let channel3 = display_text(30);
    let channel3_mv = volt_cents(display_number(6, 8));
    let channel3_leds = led_text();

    // 从电压界面回到频率界面, 再经周期界面进入电压界面, 应重新回到通道 1.
    press(S4);
    press(S4);
    press(S4);
    let back_to_channel1 = display_text(30);

    assert_regex(channel1, "^U-1\\s+3\\.25$", "电压界面: 通道 1 应显示光敏电阻电压 3.25V");
    assert_in(channel1_mv, 309..=341, "电压界面: 通道 1 电压超出 ±5% 精度要求");
    assert_regex(channel3, "^U-3\\s+1\\.80$", "电压界面: S5 后应显示电位器电压 1.80V");
    assert_in(channel3_mv, 171..=189, "电压界面: 通道 3 电压超出 ±5% 精度要求");
    assert_eq(channel1_leds, "00001000", "电压界面: 应为 L5 点亮, 其余指示灯熄灭");
    assert_eq(channel3_leds, "00001000", "电压界面: 切换通道不应改变指示灯");
    assert_regex(back_to_channel1, "^U-1\\s+3\\.25$", "电压界面: 再次进入应回到通道 1");
    channel1 + " -> " + channel3
});

// ---------------------------------------------------------------------------
// 检查点 5: S6 缓存通道 3 电压与 L1 指示 (4.4-1, 4.5-1)
// ---------------------------------------------------------------------------
ckpt(5, "S6 缓存通道 3 电压", "实时电压高于缓存点亮 L1, 低于缓存熄灭", || {
    boot(1000, 3.25, 1.80);

    // S6 在任何界面下都有效, 这里直接在频率界面缓存当前通道 3 电压.
    press(S6);
    let cached_leds = led_text();

    set_voltage(AIN3, 2.50);
    wait_measure();
    let up_leds = led_text();

    set_voltage(AIN3, 1.00);
    wait_measure();
    let down_leds = led_text();

    assert_eq(cached_leds[0], '0', "S6 缓存: 缓存后实时值未超过缓存时 L1 应熄灭");
    assert_eq(up_leds[0], '1', "S6 缓存: 实时电压高于缓存电压时 L1 应点亮");
    assert_eq(down_leds[0], '0', "S6 缓存: 实时电压低于缓存电压时 L1 应熄灭");
    cached_leds + " -> " + up_leds + " -> " + down_leds
});

// ---------------------------------------------------------------------------
// 检查点 6: S7 缓存频率与 L2 指示 (4.4-1, 4.5-2)
// ---------------------------------------------------------------------------
ckpt(6, "S7 缓存频率", "实时频率高于缓存点亮 L2, 低于缓存熄灭", || {
    boot(2035, 3.25, 1.80);

    press(S7);
    let cached_leds = led_text();

    set_frequency_hz(3000);
    wait_measure();
    let up_leds = led_text();

    set_frequency_hz(1000);
    wait_measure();
    let down_leds = led_text();

    assert_eq(cached_leds[1], '0', "S7 缓存: 缓存后实时值未超过缓存时 L2 应熄灭");
    assert_eq(up_leds[1], '1', "S7 缓存: 实时频率高于缓存频率时 L2 应点亮");
    assert_eq(down_leds[1], '0', "S7 缓存: 实时频率低于缓存频率时 L2 应熄灭");
    cached_leds + " -> " + up_leds + " -> " + down_leds
});

// ---------------------------------------------------------------------------
// 检查点 7: S4 界面循环与按键响应时间 (4.2-4, 4.4)
// ---------------------------------------------------------------------------
ckpt(7, "S4 界面循环与响应时间", "频率 -> 周期 -> 电压 -> 频率, 200ms 内完成切换", || {
    boot(1000, 3.25, 1.80);

    // tap_key 已经包含按下与释放, 这里只再等题目允许的 200ms 响应窗口.
    tap_key(S4, 60);
    wait_key_response();
    let period_text = display_text(0);

    tap_key(S4, 60);
    wait_key_response();
    let volt_text = display_text(0);

    tap_key(S4, 60);
    wait_key_response();
    let freq_text = display_text(0);

    assert_eq(period_text[0], 'N', "S4 切换: 200ms 内应切到周期界面");
    assert_eq(volt_text[0], 'U', "S4 切换: 200ms 内应切到电压界面");
    assert_eq(freq_text[0], 'F', "S4 切换: 第三次按键应回到频率界面");
    freq_text
});

// ---------------------------------------------------------------------------
// 检查点 8: S7 长按切换 LED 功能 (4.4-2, 图 7)
// ---------------------------------------------------------------------------
ckpt(8, "S7 长按切换 LED 功能", "长按后全部指示灯熄灭, 再次长按恢复", || {
    boot(1000, 3.25, 1.80);
    let before = led_text();

    long_press(S7);
    let disabled = led_text();

    set_voltage(AIN3, 2.50);
    set_frequency_hz(4000);
    wait_measure();
    let disabled_after_change = led_text();

    long_press(S7);
    let restored = led_text();

    assert_eq(before, "00100000", "S7 长按: 长按前应为 L3 点亮");
    assert_eq(disabled, "00000000", "S7 长按: 长按后全部指示灯应熄灭");
    assert_eq(disabled_after_change, "00000000", "S7 长按: LED 功能关闭期间指示灯应保持熄灭");
    assert_eq(restored, "00100000", "S7 长按: 再次长按应恢复 LED 指示功能");
    disabled + " -> " + restored
});
