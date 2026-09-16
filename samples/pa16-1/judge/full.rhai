// 第十六届蓝桥杯单片机省赛 (大学组) 程序设计试题 (85 分) 整卷评测脚本.
//
// 题目原文为扫描件, 通过本地 OCR 转成文本后整理 (见 problem/题目OCR文本.txt), 评测点按原文推导:
//   3.2 性能: 测距精度 ±3cm, 按键响应 <= 0.1s, 输出设备响应 <= 0.1s, 显示刷新 <= 0.1s.
//   3.3 光强: N1 为 VAIN >= 3.0V, N2 为 2.0V <= VAIN < 3.0V,
//             N3 为 0.5V <= VAIN < 2.0V, N4 为 VAIN < 0.5V.
//   3.4 温度: 温度高于温度参数 PC 时触发高温判定.
//   3.5 移动检测: 距离低于距离参数 PL 触发接近判定; 每 1 秒采集距离,
//             连续两次距离差值 ΔL < 5cm 为静止, 5cm <= ΔL < 10cm 为徘徊, ΔL >= 10cm 为跑动.
//   3.6 输出: 接近判定下按光强等级点亮 L1-L4; L8 指示运动状态 (跑动时 0.1s 闪烁);
//             同时满足接近与高温时继电器吸合; 其余指示灯熄灭, 蜂鸣器静音.
//   3.7 显示: 环境状态界面显示温度整数与光强等级, 运动检测界面显示运动状态与 3 位距离 (不足补 0),
//             参数界面显示温度参数与距离参数, 统计界面显示继电器吸合次数 (4 位, 高位熄灭).
//   3.8 按键: S4 循环切换四个界面, S5 在参数界面切换两个子界面, S8/S9 加减参数,
//             统计界面下 S8+S9 保持 2 秒清零吸合次数; 温度参数 20-80 摄氏度, 距离参数 20-80cm.
//   四 初始状态: 环境状态界面, 温度参数 30, 距离参数 30.

key_mode(KBD);

fn wait_measure() {
    run_ms(1200);
}

fn press(key) {
    tap_key(key, 60);
    run_ms(200);
}

// 上电并等待采集稳定: 采样实现需要若干个采集周期后输出与状态才稳定.
fn boot(temp, light_v, distance) {
    reset(POWER_RESET);
    key_mode(KBD);
    set_temperature_c(temp);
    set_voltage(AIN1, light_v);
    set_distance_cm(distance);
    run_ms(4000);
}

fn led_text() {
    let text = "";
    for index in 1..=8 {
        text += if led_on(index) { "1" } else { "0" };
    }
    text
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

// 从环境状态界面按 S4 进入指定界面: 0 环境, 1 运动, 2 参数, 3 统计.
fn goto_page(index) {
    for step in 0..index {
        press(S4);
    }
    run_ms(600);
}

// ---------------------------------------------------------------------------
// 检查点 1: 初始状态与光强等级 (3.3, 3.7-1, 四)
// ---------------------------------------------------------------------------
ckpt(1, "初始状态与光强等级", "环境状态界面显示温度整数与光强等级, 参数默认 30/30", || {
    boot(23.0, 3.5, 35);
    let n1 = display_text(30);

    set_voltage(AIN1, 2.5);
    wait_measure();
    let n2 = display_text(30);

    set_voltage(AIN1, 1.0);
    wait_measure();
    let n3 = display_text(30);

    set_voltage(AIN1, 0.2);
    wait_measure();
    let n4 = display_text(30);

    goto_page(2);
    let pc = display_text(30);
    press(S5);
    let pl = display_text(30);

    assert_regex(n1, "^C\\s*23\\s+N1$", "光强等级: 3.5V 应判为 N1 并显示温度 23");
    assert_regex(n2, "^C\\s*23\\s+N2$", "光强等级: 2.5V 应判为 N2");
    assert_regex(n3, "^C\\s*23\\s+N3$", "光强等级: 1.0V 应判为 N3");
    assert_regex(n4, "^C\\s*23\\s+N4$", "光强等级: 0.2V 应判为 N4");
    assert_regex(pc, "^PC\\s+30$", "初始状态: 温度参数应为 30");
    assert_regex(pl, "^PL\\s+30$", "初始状态: 距离参数应为 30");
    n1 + " " + n2 + " " + n3 + " " + n4
});

// ---------------------------------------------------------------------------
// 检查点 2: 灯光控制 (3.6-1)
// ---------------------------------------------------------------------------
ckpt(2, "灯光控制", "接近判定下按光强等级点亮 L1 到 L1-L4", || {
    boot(23.0, 3.5, 25);
    let n1 = led_text();
    set_voltage(AIN1, 2.5);
    wait_measure();
    let n2 = led_text();
    set_voltage(AIN1, 1.0);
    wait_measure();
    let n3 = led_text();
    set_voltage(AIN1, 0.2);
    wait_measure();
    let n4 = led_text();

    // 距离改为超过距离参数, 取消接近判定, L1-L4 应全部熄灭.
    set_distance_cm(60);
    wait_measure();
    let far = led_text();

    assert_eq(n1[0..4], "1000", "灯光控制: N1 时只应点亮 L1");
    assert_eq(n2[0..4], "1100", "灯光控制: N2 时应点亮 L1-L2");
    assert_eq(n3[0..4], "1110", "灯光控制: N3 时应点亮 L1-L3, L4 熄灭");
    assert_eq(n4[0..4], "1111", "灯光控制: N4 时应点亮 L1-L4");
    assert_eq(far[0..4], "0000", "灯光控制: 未触发接近判定时 L1-L4 应全部熄灭");
    n1[0..4] + " " + n2[0..4] + " " + n3[0..4] + " " + n4[0..4]
});

// ---------------------------------------------------------------------------
// 检查点 3: 运动检测界面与状态判定 (3.5, 3.7-2)
// ---------------------------------------------------------------------------
ckpt(3, "运动检测界面与状态判定", "距离补 0 显示, ΔL 决定静止/徘徊/跑动", || {
    boot(23.0, 3.5, 25);
    goto_page(1);
    // 进入运动界面后先等一次状态锁定结束, 上电初期的状态可能是跑动.
    run_ms(3500);
    let still = display_text(30);

    // 距离变化 7cm: 徘徊. 题目要求每 1 秒采集一次, 且状态变化后锁定 3 秒, 因此等待 3.5 秒再读.
    set_distance_cm(32);
    run_ms(3500);
    let wander = display_text(30);

    // 等待状态锁定结束后再变化 15cm: 跑动.
    run_ms(3500);
    set_distance_cm(47);
    run_ms(3500);
    let run = display_text(30);

    assert_regex(still, "^L1\\s+0\\d\\d$", "运动检测: 距离不变时应为静止且距离补 0 显示");
    assert_regex(wander, "^L2\\s+0\\d\\d$", "运动检测: 距离变化 7cm 时应为徘徊");
    assert_regex(run, "^L3\\s+0\\d\\d$", "运动检测: 距离变化 15cm 时应为跑动");
    still + " -> " + wander + " -> " + run
});

// ---------------------------------------------------------------------------
// 检查点 4: L8 运动状态指示 (3.6-2)
// ---------------------------------------------------------------------------
ckpt(4, "L8 运动状态指示", "静止熄灭, 徘徊点亮, 跑动 0.1s 闪烁", || {
    boot(23.0, 3.5, 25);
    goto_page(1);
    run_ms(3500);
    let still = sample_led(L8);

    set_distance_cm(32);
    run_ms(3500);
    let wander = sample_led(L8);

    run_ms(3500);
    set_distance_cm(47);
    run_ms(3500);
    let run = sample_led(L8);

    assert_eq(still[1], 0, "L8: 静止时 L8 应熄灭, actual=" + still[0]);
    assert(wander[0].contains("1"), "L8: 徘徊时 L8 应点亮, actual=" + wander[0]);
    assert(run[1] >= 2, "L8: 跑动时 L8 应以 0.1s 闪烁, actual=" + run[0]);
    still[0] + " | " + wander[0] + " | " + run[0]
});

// ---------------------------------------------------------------------------
// 检查点 5: 继电器控制与吸合次数 (3.6-3, 3.7-4, 3.8)
// ---------------------------------------------------------------------------
ckpt(5, "继电器控制与吸合次数", "接近且高温时吸合, 吸合次数累计并可用 S8+S9 清零", || {
    // 距离 25cm 小于距离参数 30cm (接近), 温度 35 度高于温度参数 30 (高温).
    boot(35.0, 3.5, 25);
    let closed = relay_on();

    // 温度降到 20 度, 取消高温判定, 继电器应断开.
    set_temperature_c(20.0);
    wait_measure();
    let opened = relay_on();

    // 统计界面下 S8+S9 保持 2 秒以上清零.
    goto_page(3);
    let count_before = display_number(3, 8);
    set_key(S8, true);
    set_key(S9, true);
    run_ms(2300);
    set_key(S8, false);
    set_key(S9, false);
    run_ms(500);
    let count_after = display_number(3, 8);

    assert_eq(closed, true, "继电器: 接近且高温时应吸合");
    assert_eq(opened, false, "继电器: 只有接近而不满足高温时应断开");
    assert_eq(count_before, 1, "吸合次数: 继电器吸合一次后统计应为 1");
    assert_eq(count_after, 0, "吸合次数: 统计界面下 S8+S9 保持 2 秒应清零");
    count_before.to_string() + " -> " + count_after.to_string()
});

// ---------------------------------------------------------------------------
// 检查点 6: 参数加减与范围 (3.8, 3.8-2)
// ---------------------------------------------------------------------------
ckpt(6, "参数加减与范围", "温度参数 ±1 摄氏度, 距离参数 ±5cm, 范围 20-80", || {
    boot(23.0, 3.5, 35);
    goto_page(2);
    let pc = display_text(30);
    press(S8);
    let pc_up = display_text(30);
    press(S9);
    press(S9);
    let pc_down = display_text(30);

    press(S5);
    let pl = display_text(30);
    press(S8);
    let pl_up = display_text(30);

    // 把距离参数加到上限 80 后继续加应保持 80.
    for step in 0..12 {
        press(S8);
    }
    let pl_max = display_text(30);

    assert_regex(pc, "^PC\\s+30$", "参数界面: 温度参数默认应为 30");
    assert_regex(pc_up, "^PC\\s+31$", "参数界面: 温度参数加 1 后应为 31");
    assert_regex(pc_down, "^PC\\s+29$", "参数界面: 温度参数减两次后应为 29");
    assert_regex(pl, "^PL\\s+30$", "参数界面: 距离参数默认应为 30");
    assert_regex(pl_up, "^PL\\s+35$", "参数界面: 距离参数加 1 次后应为 35");
    assert_regex(pl_max, "^PL\\s+80$", "参数界面: 距离参数上限应为 80");
    pc + ", " + pl + ", " + pl_max
});

// ---------------------------------------------------------------------------
// 检查点 7: 其余指示灯与蜂鸣器 (3.6-4)
// ---------------------------------------------------------------------------
ckpt(7, "其余指示灯与蜂鸣器", "L5-L7 熄灭, 蜂鸣器静音", || {
    boot(35.0, 0.2, 25);
    let leds = led_text();
    let buzzer = buzzer_on();

    goto_page(1);
    run_ms(1000);
    let leds_after = led_text();

    assert_eq(leds[4..7], "000", "指示灯: L5-L7 应保持熄灭");
    assert_eq(leds_after[4..7], "000", "指示灯: 切换界面后 L5-L7 仍应熄灭");
    assert_eq(buzzer, false, "蜂鸣器: 应保持静音");
    leds + " -> " + leds_after
});

// ---------------------------------------------------------------------------
// 检查点 8: 按键与显示响应时间 (3.2)
// ---------------------------------------------------------------------------
ckpt(8, "按键与显示响应时间", "按键动作响应 <= 0.1s, 按键后数码管在 0.1s 内更新", || {
    boot(23.0, 3.5, 25);

    // S4: 环境状态界面 -> 运动检测界面, 界面标识应为 L.
    tap_key(S4, 40);
    let s4_ms = 0;
    while s4_ms < 1000 && display_text(10)[0..1] != "L" {
        run_ms(10);
        s4_ms += 10;
    }
    let motion_page = display_text(30);

    // S4 再到参数界面 (默认温度参数 PC), 在参数界面按 S5 切到距离参数 PL, 也应很快更新.
    press(S4);
    tap_key(S5, 40);
    let s5_ms = 0;
    while s5_ms < 1000 && display_text(10)[0..2] != "PL" {
        run_ms(10);
        s5_ms += 10;
    }
    let param_page = display_text(30);

    assert_in(s4_ms, 0..=100, "响应时间: 按键响应应在 0.1 秒内, 实际 " + s4_ms.to_string() + "ms, 页面 " + motion_page);
    assert_in(s5_ms, 0..=100, "响应时间: 按键后显示应在 0.1 秒内更新, 实际 " + s5_ms.to_string() + "ms, 页面 " + param_page);
    s4_ms.to_string() + "ms, " + s5_ms.to_string() + "ms"
});
