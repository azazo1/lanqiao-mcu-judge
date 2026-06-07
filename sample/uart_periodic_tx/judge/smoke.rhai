fn assert_sent_count(expected, label) {
    let text = display_text(30);
    assert_eq(text[0..2], expected, label + ": 发送计数显示错误");
}

assert_eq(uart_take(), "", "上电检查: UART1 默认输出错误");

run_ms(1000);
assert_sent_count("04", "整队列读取前");
assert_eq(
    uart_take(),
    "OKERROROKERROR",
    "整队列读取: 长时间累计后应合并为 1 段"
);
assert_eq(uart_take(), "", "整队列读取: 缓冲区清空错误");

reset(POWER_RESET);
assert_eq(uart_take(), "", "复位后: UART1 默认输出错误");

run_ms(1000);
assert_sent_count("04", "分段读取前");
assert_eq(uart_peek(), "OKERROROKERROR", "peek 全量读取错误");
assert_eq(uart_peek(10), "OK", "peek 首段读取错误");
assert_eq(uart_take(10), "OK", "分段读取: 第 1 段错误");
assert_eq(uart_peek(10), "ERROR", "分段读取后 peek 第 2 段错误");
assert_eq(uart_take(10), "ERROR", "分段读取: 第 2 段错误");
assert_eq(uart_take(10), "OK", "分段读取: 第 3 段错误");
assert_eq(uart_take(10), "ERROR", "分段读取: 第 4 段错误");
assert_eq(uart_take(10), "", "分段读取: 缓冲区清空错误");
assert_eq(uart_peek(), "", "分段读取: peek 清空错误");
