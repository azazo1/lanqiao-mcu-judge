fn settle() {
    run_ms(320);
}

fn assert_query(query, expected, label) {
    uart_write(query);
    run_ms(160);
    assert_eq(uart_take(), expected, label);
}

set_rtc(23, 59, 50);
settle();

assert_query("(F,?)", "(F,2000)", "查询默认频率参数");
assert_query("(S,?)", "(S,2)", "查询默认容器类型");
assert_query("(H1,?)", "(H1,1.0)", "查询默认液位上限");
assert_query("(H2,?)", "(H2,0.1)", "查询默认液位下限");
assert_query("(r,?)", "(r,1.0)", "查询默认半径");
assert_query("(L,?)", "(L,1.0)", "查询默认长度");
assert_query("(W,?)", "(W,1.0)", "查询默认宽度");
assert_query("(H,?)", "(H,1.0)", "查询默认高度");

uart_write("(F,1500)");
run_ms(160);
assert_eq(uart_take(), "OK", "设置频率参数");
assert_query("(F,?)", "(F,1500)", "查询更新后的频率参数");

uart_write("(H1,1.8)");
run_ms(160);
assert_eq(uart_take(), "OK", "设置液位上限参数");
assert_query("(H1,?)", "(H1,1.8)", "查询更新后的液位上限");

uart_write("(H2,0.5)");
run_ms(160);
assert_eq(uart_take(), "OK", "设置液位下限参数");
assert_query("(H2,?)", "(H2,0.5)", "查询更新后的液位下限");

uart_write("(L,1.5)(H,2.0)");
run_ms(160);
assert_eq(uart_take(), "OK", "成组设置尺寸参数");
assert_query("(L,?)", "(L,1.5)", "查询更新后的长度");
assert_query("(H,?)", "(H,2.0)", "查询更新后的高度");

uart_write("(W,2.0)(S,3)");
run_ms(160);
assert_eq(uart_take(), "WARN", "部分生效参数应返回 WARN");
assert_query("(W,?)", "(W,2.0)", "WARN 后有效参数应生效");
assert_query("(S,?)", "(S,2)", "WARN 后无效容器类型不应生效");

uart_write("(S,1)");
run_ms(160);
assert_eq(uart_take(), "OK", "合法容器类型设置");
assert_query("(S,?)", "(S,1)", "查询更新后的容器类型");

uart_write("(S,4)");
run_ms(160);
assert_eq(uart_take(), "ERROR", "非法容器类型应返回 ERROR");
assert_query("(S,?)", "(S,1)", "非法容器类型不应改写原值");

uart_write("(T:121315)");
run_ms(160);
assert_eq(uart_take(), "OK", "设置合法时间");
assert_eq(display_text(30), "12-13-15", "设置合法时间后显示");

uart_write("(T:233344)");
run_ms(160);
assert_eq(uart_take(), "OK", "再次设置合法时间");
assert_eq(display_text(30), "23-33-44", "再次设置合法时间后显示");

uart_write("(T:286344)");
run_ms(160);
assert_eq(uart_take(), "ERROR", "非法时间应返回 ERROR");
assert_eq(display_text(30), "23-33-44", "非法时间后显示保持不变");
