#[derive(Debug, Clone)]
pub struct JudgeApiItem {
    pub name: &'static str,
    pub signature: &'static str,
    pub description: &'static str,
    pub snippet: &'static str,
}

pub fn judge_api_catalog() -> &'static [JudgeApiItem] {
    &[
        JudgeApiItem {
            name: "ckpt",
            signature: "ckpt(index, condition, expected, || { ... })",
            description: "定义一个可追踪的评测点",
            snippet: "ckpt(1, \"测试条件\", \"期望结果\", || {\n    true\n});",
        },
        JudgeApiItem {
            name: "run_ms",
            signature: "run_ms(ms)",
            description: "推进指定毫秒数",
            snippet: "run_ms(100);",
        },
        JudgeApiItem {
            name: "run_to",
            signature: "run_to(target, edge, timeout_ns)",
            description: "等待信号边沿",
            snippet: "let dt = run_to(L1, UP, 100_000_000);",
        },
        JudgeApiItem {
            name: "run_to_state",
            signature: "run_to_state(target, expected, timeout_ns)",
            description: "等待状态达到期望值",
            snippet: "run_to_state(\"seg.d1.visible\", true, 100_000_000);",
        },
        JudgeApiItem {
            name: "run_to_event",
            signature: "run_to_event(track, timeout_ns)",
            description: "等待协议或显示事件",
            snippet: "let event = run_to_event(\"seg.change\", 100_000_000);",
        },
        JudgeApiItem {
            name: "display_text",
            signature: "display_text(window_ms)",
            description: "读取稳定数码管文本",
            snippet: "let text = display_text(30);",
        },
        JudgeApiItem {
            name: "display_number",
            signature: "display_number(start, end, window_ms)",
            description: "读取指定数码管范围内的数值",
            snippet: "let value = display_number(4, 8, 30);",
        },
        JudgeApiItem {
            name: "tap_key",
            signature: "tap_key(key, hold_ms)",
            description: "模拟一次按下和释放",
            snippet: "tap_key(S4, 80);",
        },
        JudgeApiItem {
            name: "set_voltage",
            signature: "set_voltage(channel, voltage)",
            description: "设置模拟输入电压",
            snippet: "set_voltage(RD1, 2.5);",
        },
        JudgeApiItem {
            name: "uart_write",
            signature: "uart_write(text)",
            description: "向 UART1 注入文本",
            snippet: "uart_write(\"(F,?)\");",
        },
    ]
}
