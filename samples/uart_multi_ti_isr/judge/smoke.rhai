fn settle() {
    run_ms(220);
}

settle();

let boot = display_text(30);
assert_eq(len(boot), 9, "上电检查: 显示长度错误");
assert_eq(boot[0..4], "000.", "上电检查: TI 计数显示错误");
assert_eq(boot[4..9], "00000", "上电检查: 数值显示错误");
assert_eq(uart_take(), "", "上电检查: 串口默认输出错误");

uart_write("0");
settle();

let text = display_text(30);
assert_eq(len(text), 9, "发送字符后: 显示长度错误");
assert_eq(text[3..4], ".", "发送字符后: TI 计数小数点显示错误");
assert_eq(text[4..9], "00000", "发送字符后: 原值显示错误");
assert_in(parse_int(text[0..3]), 2..=999, "发送字符后: TI 重入计数超出范围");
assert_eq(uart_take(), "1", "发送字符后: 串口回传错误");
assert_eq(uart_take(), "", "发送字符后: 串口缓冲清空错误");
