fn settle() {
    run_ms(220);
}

settle();

let boot = display_text(30);
assert_eq(len(boot), 9, "上电显示长度");
assert_eq(boot[0..4], "000.", "上电 TI 计数");
assert_eq(boot[4..9], "00000", "上电显示数值");
assert_eq(uart_take(), "", "上电默认无串口输出");

uart_write("0");
settle();

let text = display_text(30);
assert_eq(len(text), 9, "发送后显示长度");
assert_eq(text[3..4], ".", "发送后 TI 计数小数点");
assert_eq(text[4..9], "00000", "发送后原值显示");
assert_in(parse_int(text[0..3]), 2..=999, "发送后 TI 应重复触发");
assert_eq(uart_take(), "1", "发送后串口应回 1");
assert_eq(uart_take(), "", "读取后串口缓冲应为空");
