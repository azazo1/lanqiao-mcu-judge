// 第十七届蓝桥杯单片机省赛 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 评测点全部按题目原文推导:
//   3.2 性能: 模拟信号采样 <= 0.2s, 按键响应 <= 0.1s, LED 响应 <= 0.1s.
//   3.3 压力: 由 PCF8591 采集电位器电压后线性换算, 零点校准值 PL 与满量程校准值 PH 在工厂模式可调;
//             当前压力比 3 秒前低 1.5kPa 以上时触发泄压事件并记录时间.
//   3.4 显示: 运行界面 时:分 + 压力 (1 位小数), 地址设置界面 "AE" + 地址,
//             泄压记录界面 "CC" + 泄压时间或 "-", 工厂模式 "PH"/"PL" + 校准值 (1 位小数).
//   3.5 按键: S4 切换运行/地址设置/泄压记录; S5 长按 3 秒进出工厂模式, 工厂模式内短按切换 PH/PL;
//             S8/S9 在地址界面减加地址, 在工厂模式减加 0.1; S12 保存参数到 EEPROM;
//             S13 在泄压记录界面短按清除记录, 任一界面长按 3 秒恢复出厂值.
//   3.6 EEPROM: 零点校准值 @0x01, 满量程校准值 @0x02 (均按 10 倍存储), 设备地址 @0x03,
//             特征字 @0x00, 不等于 0xAA 时按首次上电写入 9.0/0.0/8 与 0xAA.
//   3.7 通信: 串口 9600 8N1, 命令 "#设备地址?" 返回 "压力数值@时间(时:分)", 地址不匹配则静默.
//   3.8 LED: L1 收到本机查询后以 0.2 秒闪烁 3 秒, L2 工厂模式以 0.2 秒闪烁, L3 泄压后常亮直至清除记录.

key_mode(KBD);

fn uart_setup() {
    uart_config(8, 9600, 1, "none");
}

fn press(key) {
    tap_key(key, 60);
    run_ms(300);
}

fn long_press(key) {
    set_key(key, true);
    run_ms(3300);
    set_key(key, false);
    run_ms(400);
}

// 上电: EEPROM 中已有特征字与参数, 时间与电位器电压按给定值设置.
// 注意采样实现会在启动早期读取 EEPROM, 因此参数必须在复位之前写入.
fn boot(character, pl_x10, ph_x10, address, ain3, hour, minute) {
    set_eeprom(0, character);
    set_eeprom(1, pl_x10);
    set_eeprom(2, ph_x10);
    set_eeprom(3, address);
    reset(POWER_RESET);
    key_mode(KBD);
    uart_setup();
    set_voltage(AIN3, ain3);
    run_ms(2500);
    set_rtc(hour, minute, 0);
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

// 运行界面: 前 4 位是时与分, 之后的数字才是压力值.
fn run_pressure_x10() {
    let text = display_text(30);
    let tail = text[4..];
    let digits = "";
    for pos in 0..tail.len() {
        let ch = tail[pos];
        if (ch >= '0' && ch <= '9') || ch == '.' {
            digits += ch;
        }
    }
    if digits.len() == 0 {
        0
    } else {
        (parse_float(digits) * 10.0).round().to_int()
    }
}

// 取显示文本中的数字部分, 返回放大 10 倍后的整数 (压力与校准值都是 kPa, 均保留 1 位小数).
fn display_deci_x10() {
    let text = display_text(30);
    let digits = "";
    for pos in 0..text.len() {
        let ch = text[pos];
        if (ch >= '0' && ch <= '9') || ch == '.' {
            digits += ch;
        }
    }
    if digits.len() == 0 {
        0
    } else {
        (parse_float(digits) * 10.0).round().to_int()
    }
}

// ---------------------------------------------------------------------------
// 检查点 1: 首次上电与运行界面 (3.4-1, 3.6-3, 四)
// ---------------------------------------------------------------------------
ckpt(1, "首次上电与运行界面", "特征字不等于 0xAA 时写入出厂值, 运行界面显示时间与压力", || {
    boot(0x00, 0, 0, 0, 2.5, 22, 3);
    let run_text = display_text(30);
    let pressure = run_pressure_x10();
    let char_word = eeprom_byte(0);
    let pl = eeprom_byte(1);
    let ph = eeprom_byte(2);
    let addr = eeprom_byte(3);

    assert_regex(run_text, "^2203\\s+\\d+\\.\\d$", "运行界面: 应显示 22 时 03 分与一位小数的压力值");
    assert_in(pressure, 43..=47, "压力换算: 2.5V 在 0.0-9.0kPa 标定下应约为 4.5kPa");
    assert_eq(char_word, 0xAA, "首次上电: 特征字应写入 0xAA");
    assert_eq(pl, 0, "首次上电: 零点校准值应写入 0.0kPa (存储值 0)");
    assert_eq(ph, 90, "首次上电: 满量程校准值应写入 9.0kPa (存储值 90)");
    assert_eq(addr, 8, "首次上电: 设备地址应写入 8");
    run_text
});

// ---------------------------------------------------------------------------
// 检查点 2: EEPROM 参数上电读取 (3.6)
// ---------------------------------------------------------------------------
ckpt(2, "EEPROM 参数上电读取", "特征字已是 0xAA 时, 上电应按 EEPROM 中的校准值与设备地址工作", || {
    // EEPROM 中: 特征字 0xAA, 零点校准值 1.2kPa (12), 满量程校准值 3.4kPa (34), 地址 56.
    boot(0xAA, 12, 34, 56, 2.5, 22, 3);
    let run_text = display_text(30);
    let pressure = run_pressure_x10();
    let stored = eeprom_byte(3);

    press(S4);
    let addr_page = display_text(30);

    long_press(S5);
    let ph_page = display_text(30);
    press(S5);
    let pl_page = display_text(30);
    long_press(S5);

    assert_regex(addr_page, "^AE\\s+56$", "EEPROM 读取: 上电应按地址 0x03 中的 56 工作");
    assert_regex(ph_page, "^PH\\s+-?3\\.4$", "EEPROM 读取: 上电应按地址 0x02 中的 3.4kPa 作为满量程校准值");
    assert_regex(pl_page, "^PL\\s+-?1\\.2$", "EEPROM 读取: 上电应按地址 0x01 中的 1.2kPa 作为零点校准值");
    // 2.5V 在 1.2-3.4kPa 标定下应约为 2.3kPa.
    assert_in(pressure, 21..=25, "EEPROM 读取: 压力换算应使用读回的校准值");
    assert_eq(stored, 56, "EEPROM 读取: 特征字有效时不应改写设备地址");
    run_text + " " + addr_page + " " + ph_page + " " + pl_page
});

// ---------------------------------------------------------------------------
// 检查点 3: 压力线性换算 (3.3)
// ---------------------------------------------------------------------------
ckpt(3, "压力线性换算", "电压由 0V 升到 5V 时压力由零点校准值线性升到满量程校准值", || {
    // 使用出厂默认标定 (零点 0.0kPa, 满量程 9.0kPa) 考察换算本身.
    boot(0x00, 0, 0, 0, 0.0, 22, 3);
    let zero = run_pressure_x10();

    set_voltage(AIN3, 1.25);
    run_ms(1000);
    let quarter = run_pressure_x10();

    set_voltage(AIN3, 2.5);
    run_ms(1000);
    let half = run_pressure_x10();

    set_voltage(AIN3, 3.75);
    run_ms(1000);
    let three_quarter = run_pressure_x10();

    set_voltage(AIN3, 5.0);
    run_ms(1000);
    let full = run_pressure_x10();

    assert_in(zero, 0..=2, "压力换算: 0V 应对应零点校准值 0.0kPa");
    assert_in(quarter, 21..=24, "压力换算: 1.25V 应约为 2.25kPa");
    assert_in(half, 43..=47, "压力换算: 2.5V 应约为 4.5kPa");
    assert_in(three_quarter, 65..=70, "压力换算: 3.75V 应约为 6.75kPa");
    assert_in(full, 88..=92, "压力换算: 5V 应对应满量程校准值 9.0kPa");
    zero.to_string() + " " + quarter.to_string() + " " + half.to_string() + " " + three_quarter.to_string() + " " + full.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 3: 界面切换与地址设置 (3.4-2, 3.4-3, 3.5, 3.6-2)
// ---------------------------------------------------------------------------
ckpt(4, "界面切换与地址设置", "S4 在三个界面间切换, S8/S9 调整地址, S12 保存到 EEPROM", || {
    boot(0x00, 0, 0, 0, 2.5, 22, 3);

    press(S4);
    let addr_page = display_text(30);
    press(S9);
    let addr_up = display_text(30);
    press(S8);
    press(S8);
    let addr_down = display_text(30);
    press(S12);
    run_ms(400);
    let saved = eeprom_byte(3);

    press(S4);
    let record_page = display_text(30);

    assert_regex(addr_page, "^AE\\s+8$", "地址设置界面: 应以 AE 为标识显示地址 8");
    assert_regex(addr_up, "^AE\\s+9$", "地址设置界面: S9 应把地址加到 9");
    assert_regex(addr_down, "^AE\\s+7$", "地址设置界面: 连减两次后地址应为 7");
    assert_eq(saved, 7, "EEPROM: S12 应把地址 7 保存到 0x03");
    assert_regex(record_page, "^CC\\s+-$", "泄压记录界面: 未发生泄压事件时应显示 CC 与 -");
    addr_page + " " + addr_up + " " + addr_down + " " + record_page
});

// ---------------------------------------------------------------------------
// 检查点 4: 工厂模式与校准值保存 (3.3-1, 3.4-4, 3.5, 3.6-1)
// ---------------------------------------------------------------------------
ckpt(5, "工厂模式与校准值", "长按 S5 进出工厂模式, S5 切换 PH/PL, S8/S9 调整 0.1, S12 保存并生效", || {
    boot(0x00, 0, 0, 0, 5.0, 22, 3);

    long_press(S5);
    let ph_page = display_text(30);
    press(S8);
    let ph_down = display_text(30);
    press(S9);
    press(S9);
    let ph_up = display_text(30);
    press(S12);
    run_ms(400);
    let ph_stored = eeprom_byte(2);

    press(S5);
    let pl_page = display_text(30);
    press(S9);
    let pl_up = display_text(30);
    press(S12);
    run_ms(400);
    let pl_stored = eeprom_byte(1);

    long_press(S5);
    run_ms(800);
    let pressure = run_pressure_x10();

    assert_regex(ph_page, "^PH\\s+-?9\\.0$", "工厂模式: 进入时应显示满量程校准值 9.0kPa");
    assert_regex(ph_down, "^PH\\s+-?8\\.9$", "工厂模式: 减一次后应为 8.9kPa");
    assert_regex(ph_up, "^PH\\s+-?9\\.1$", "工厂模式: 再加两次后应为 9.1kPa");
    assert_eq(ph_stored, 91, "EEPROM: 满量程校准值应按 10 倍存储为 91");
    assert_regex(pl_page, "^PL\\s+-?0\\.0$", "工厂模式: 短按 S5 后应显示零点校准值 0.0kPa");
    assert_regex(pl_up, "^PL\\s+-?0\\.1$", "工厂模式: 零点校准值加一次后应为 0.1kPa");
    assert_eq(pl_stored, 1, "EEPROM: 零点校准值应按 10 倍存储为 1");
    assert_in(pressure, 90..=92, "校准值生效: 5V 在 0.1-9.1kPa 标定下应约为 9.1kPa");
    ph_page + " " + ph_up + " " + pl_page + " " + pl_up
});

// ---------------------------------------------------------------------------
// 检查点 5: 串口通信 (3.7, 3.8-1)
// ---------------------------------------------------------------------------
ckpt(6, "串口通信", "#设备地址? 返回 压力@时间, 地址不匹配时静默, 通信时 L1 闪烁", || {
    boot(0x00, 0, 0, 0, 2.5, 23, 5);

    uart_write("#008?");
    run_ms(500);
    let reply = uart_take(80);
    let comm_led = sample_led(L1);

    uart_write("#009?");
    run_ms(500);
    let wrong = uart_take(80);

    assert_regex(reply, "^4\\.5kPa@23:05$", "串口: 应返回 压力数值@时分, 例如 4.5kPa@23:05");
    assert(comm_led[1] >= 2, "通信指示: 收到本机查询后 L1 应以 0.2 秒闪烁, actual=" + comm_led[0]);
    assert_eq(wrong.len(), 0, "串口: 设备地址不匹配时应保持静默, actual=" + wrong);
    reply + " | " + wrong
});

// ---------------------------------------------------------------------------
// 检查点 6: 泄压记录与报警 (3.3-2, 3.4-3, 3.5, 3.8-3)
// ---------------------------------------------------------------------------
ckpt(7, "泄压记录与报警", "压力骤降触发泄压事件记录时间并点亮 L3, S13 短按清除记录", || {
    boot(0xAA, 0, 90, 8, 5.0, 14, 20);
    // 保持高压力, 然后骤降到低压 (降幅超过 1.5kPa).
    run_ms(3500);
    set_voltage(AIN3, 1.0);
    run_ms(1200);

    press(S4);
    press(S4);
    let record_page = display_text(30);
    let alarm_leds = led_text();

    press(S13);
    let cleared_page = display_text(30);
    let cleared_leds = led_text();

    assert_regex(record_page, "^CC\\s+142\\d$", "泄压记录: 应记录泄压发生的时间 (14 时 2x 分)");
    assert_eq(alarm_leds[2], '1', "泄压报警: 泄压事件后 L3 应常亮");
    assert_regex(cleared_page, "^CC\\s+-$", "泄压记录: S13 短按应清除记录");
    assert_eq(cleared_leds[2], '0', "泄压报警: 清除记录后 L3 应熄灭");
    record_page + " -> " + cleared_page
});

// ---------------------------------------------------------------------------
// 检查点 7: 出厂值恢复与工厂模式指示灯 (3.5, 3.8-2)
// ---------------------------------------------------------------------------
ckpt(8, "出厂值恢复与工厂模式指示", "长按 S13 恢复出厂值并写入 EEPROM, 工厂模式下 L2 闪烁", || {
    boot(0xAA, 12, 55, 23, 2.5, 22, 3);

    long_press(S13);
    run_ms(500);
    let run_text = display_text(30);
    let pl = eeprom_byte(1);
    let ph = eeprom_byte(2);
    let addr = eeprom_byte(3);

    long_press(S5);
    let factory_page = display_text(30);
    let factory_led = sample_led(L2);
    long_press(S5);
    let after_exit = led_text();

    assert_regex(run_text, "^2203\\s+", "出厂值恢复: 应跳转到运行界面");
    assert_eq(pl, 0, "出厂值恢复: 零点校准值应恢复为 0.0kPa");
    assert_eq(ph, 90, "出厂值恢复: 满量程校准值应恢复为 9.0kPa");
    assert_eq(addr, 8, "出厂值恢复: 设备地址应恢复为 8");
    assert_regex(factory_page, "^PH\\s|^PL\\s", "工厂模式指示: 长按 S5 应进入工厂模式界面");
    assert(factory_led[1] >= 2, "工厂模式指示: L2 应以 0.2 秒闪烁, actual=" + factory_led[0]);
    assert_eq(after_exit[1], '0', "工厂模式指示: 退出工厂模式后 L2 应熄灭");
    run_text + " " + factory_page + " " + factory_led[0]
});

// ---------------------------------------------------------------------------
// 检查点 9: 响应时间要求 (3.2)
// ---------------------------------------------------------------------------
ckpt(9, "响应时间要求", "模拟信号采样 <= 0.2s, 按键响应 <= 0.1s, 指示灯响应 <= 0.1s", || {
    boot(0x00, 0, 0, 0, 2.5, 22, 3);

    // 采样: 电位器电压改到 5.0V 后 (对应满量程 9.0kPa), 运行界面应在 0.2 秒内更新压力值.
    set_voltage(AIN3, 5.0);
    let sample_ms = 0;
    while sample_ms < 2000 && run_pressure_x10() < 89 {
        run_ms(10);
        sample_ms += 10;
    }
    let sample_page = display_text(30);

    // 按键响应: 按 S4 切到地址设置界面 (标识 AE), 应在 0.1 秒内完成.
    tap_key(S4, 40);
    let key_ms = 0;
    while key_ms < 1000 && display_text(10)[0..2] != "AE" {
        run_ms(10);
        key_ms += 10;
    }
    let key_page = display_text(30);

    // 指示灯响应: 发出一条本机查询命令后, 通信指示灯 L1 应在 0.1 秒内点亮.
    uart_write("#008?");
    let led_ms = 0;
    while led_ms < 1000 && !led_on(L1) {
        run_ms(10);
        led_ms += 10;
    }
    let reply = uart_take(80);

    assert_in(sample_ms, 0..=200, "响应时间: 模拟信号采样应在 0.2 秒内, 实际 " + sample_ms.to_string() + "ms, 页面 " + sample_page);
    assert_in(key_ms, 0..=100, "响应时间: 按键响应应在 0.1 秒内, 实际 " + key_ms.to_string() + "ms, 页面 " + key_page);
    assert_in(led_ms, 0..=100, "响应时间: 指示灯响应应在 0.1 秒内, 实际 " + led_ms.to_string() + "ms, 回复 " + reply);
    sample_ms.to_string() + "ms, " + key_ms.to_string() + "ms, " + led_ms.to_string() + "ms"
});
