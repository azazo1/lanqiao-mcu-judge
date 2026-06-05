fn settle() {
    run_ms(220);
}

fn assert_display_suffix(expected, label) {
    let text = display_text(30);
    assert_eq(len(text), 9, label + ", 显示长度");
    assert_eq(text[0..4], "000.", label + ", UART1 TI 计数不应被 UART2 改动");
    assert_eq(text[4..9], expected, label + ", 后 5 位补零显示");
}

fn assert_uart2_reply_raw(expected, label) {
    let actual = uart2_take_raw();
    assert_eq(len(actual), len(expected), label + ", UART2 原始符号数量");
    for index in 0..len(expected) {
        assert_eq(actual[index], expected[index], label + ", UART2 原始符号内容");
    }
    assert_eq(len(uart2_take_raw()), 0, label + ", UART2 读取后原始缓冲应为空");
}

settle();
assert_display_suffix("00000", "上电默认显示");
assert_eq(uart_take(), "", "上电默认无 UART1 输出");
assert_eq(len(uart2_take_raw()), 0, "上电默认无 UART2 原始输出");

uart2_config(9, 115200, 1, "none");
uart2_write_raw([0x041, 0x155, 0x0AA, 0x1F0]);
settle();

assert_display_suffix("00000", "UART2 9位输入后不改数值显示");
assert_eq(uart_take(), "", "UART2 收发不应污染 UART1 输出");
assert_uart2_reply_raw([0x1F0, 0x0AA, 0x155, 0x041], "UART2 9位反转输出");
