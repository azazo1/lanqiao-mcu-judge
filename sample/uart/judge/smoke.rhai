fn settle() {
    run_ms(220);
}

fn assert_display(expected, label) {
    let text = display_text(30);
    assert_eq(text[0..3], "   ", label + ", 前 3 位空白");
    assert_eq(text[3..8], expected, label + ", 右 5 位补零显示");
}

fn assert_uart_reply(expected, label) {
    assert_eq(uart_take(), expected, label + ", 串口输出");
    assert_eq(uart_take(), "", label + ", 读取后串口缓冲应为空");
}

settle();
assert_display("00000", "上电默认显示");
assert_eq(uart_take(), "", "上电默认无串口输出");

uart_write("0");
settle();
assert_display("00000", "输入 0 后显示原值");
assert_uart_reply("1", "输入 0");

uart_write("42");
settle();
assert_display("00042", "输入多位数后显示原值");
assert_uart_reply("43", "输入 42");

uart_write("00012");
settle();
assert_display("00012", "输入前导零后显示原值");
assert_uart_reply("13", "输入 00012");

uart_write("12a");
settle();
assert_display("00012", "非法输入后保持上次有效显示");
assert_uart_reply("Error", "输入 12a");

uart_write("7 ");
settle();
assert_display("00012", "尾随空白输入后仍保持上次有效显示");
assert_uart_reply("Error", "输入 7 加尾随空白");
