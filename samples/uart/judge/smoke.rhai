fn settle() {
    run_ms(220);
}

fn assert_display(expected_count, expected, label) {
    let text = display_text(30);
    assert_eq(len(text), 9, label + ": 显示长度错误");
    assert_eq(text[0..4], expected_count, label + ": 前4位 TI 计数显示错误");
    assert_eq(text[4..9], expected, label + ": 后5位数值显示错误");
}

fn assert_uart_reply(expected, label) {
    assert_eq(uart_take(), expected, label + ": 串口输出错误");
    assert_eq(uart_take(), "", label + ": 串口缓冲清空错误");
}

settle();
assert_display("000.", "00000", "上电默认显示");
assert_eq(uart_take(), "", "上电检查: 串口默认输出错误");

uart_write("0");
settle();
assert_display("001.", "00000", "输入 0 后显示原值");
assert_uart_reply("1", "输入 0");

uart_write("42");
settle();
assert_display("003.", "00042", "输入多位数后显示原值");
assert_uart_reply("43", "输入 42");

uart_write("00012");
settle();
assert_display("005.", "00012", "输入前导零后显示原值");
assert_uart_reply("13", "输入 00012");

uart_write("12a");
settle();
assert_display("010.", "00012", "非法输入后保持上次有效显示");
assert_uart_reply("Error", "输入 12a");

uart_write("7 ");
settle();
assert_display("015.", "00012", "尾随空白输入后仍保持上次有效显示");
assert_uart_reply("Error", "输入 7 加尾随空白");
