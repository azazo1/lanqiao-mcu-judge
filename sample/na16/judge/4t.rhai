key_mode(KEYBOARD);

fn settle() {
    run_ms(120);
}

fn page_settle() {
    run_ms(160);
}

fn sensor_settle() {
    run_ms(1600);
}

fn bool_text(value) {
    if value {
        "on"
    } else {
        "off"
    }
}

fn read_display() {
    display_text(30)
}

fn led_summary() {
    "L1=" + bool_text(led_on(L1))
        + ",L2=" + bool_text(led_on(L2))
        + ",L3=" + bool_text(led_on(L3))
        + ",L4=" + bool_text(led_on(L4))
        + ",L5=" + bool_text(led_on(L5))
        + ",L6=" + bool_text(led_on(L6))
        + ",L7=" + bool_text(led_on(L7))
        + ",L8=" + bool_text(led_on(L8))
}

fn relay_summary() {
    "relay=" + bool_text(relay_on())
}

fn ensure_sig_jumper() {
    if !jumper_installed(NET_SIG, SIG_OUT) {
        jumper_on(NET_SIG, SIG_OUT);
    }
}

fn boot_clean() {
    set_frequency_hz(0);
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    run_ms(20);
}

fn boot_with_sig() {
    set_frequency_hz(0);
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    ensure_sig_jumper();
    run_ms(20);
}

fn tap_times(key, count, delay_ms) {
    let i = 0;
    while i < count {
        tap_key(key, 80);
        i += 1;
    }
    run_ms(delay_ms);
}

fn open_data_page(index) {
    tap_times(S5, index, 160);
}

fn open_param_page(index) {
    tap_key(S4, 80);
    settle();
    tap_times(S5, index - 1, 320);
}

fn open_std_page(index) {
    tap_key(S4, 80);
    settle();
    tap_key(S4, 80);
    settle();
    tap_times(S5, index, 320);
}

fn next_page() {
    tap_key(S5, 80);
    page_settle();
}

fn adjust_up(steps) {
    tap_times(S8, steps, 320);
}

fn adjust_down(steps) {
    tap_times(S9, steps, 320);
}

fn send_uart(text) {
    uart_take();
    uart_write(text);
    run_ms(160);
}

fn query_uart(query) {
    send_uart(query);
    uart_take()
}

fn assert_int_window(actual, min, max, label) {
    assert_in(actual, min..=max, label + ": 数值超出范围");
}

fn assert_float_window(actual, min, max, label) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际" + actual
    );
}

fn assert_int_window_view(actual, min, max, label, view) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际" + actual + ", 显示=" + view
    );
}

fn assert_float_window_view(actual, min, max, label, view) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际" + actual + ", 显示=" + view
    );
}

fn extract_status_reply(reply, label) {
    if reply.contains("ERROR") {
        "ERROR"
    } else if reply.contains("WARN") {
        "WARN"
    } else if reply.contains("OK") {
        "OK"
    } else {
        assert(false, label + ": 未找到 OK/WARN/ERROR, actual=" + reply);
        ""
    }
}

fn assert_status_reply(reply, expected, label) {
    let actual = extract_status_reply(reply, label);
    assert_eq(actual, expected, label + ": 串口回复错误");
    actual
}

fn assert_reply_contains(reply, expected, label) {
    assert(
        reply.contains(expected),
        label + ": 串口查询结果错误, expected=" + expected + ", actual=" + reply
    );
    expected
}

fn extract_bracket_packet(reply, prefix, label) {
    let start = reply.index_of(prefix);
    assert(start >= 0, label + ": 未找到目标串口片段, actual=" + reply);
    let end = reply.index_of("]", start);
    assert(end >= 0, label + ": 串口片段缺少右括号, actual=" + reply);
    reply.sub_string(start, end - start + 1)
}

fn assert_query(query, expected, label) {
    send_uart(query);
    let reply = uart_take();
    assert_reply_contains(reply, expected, label)
}

fn assert_bool_state(actual, expected, label) {
    assert(
        actual == expected,
        label + ": 状态错误, expected=" + bool_text(expected) + ", actual=" + bool_text(actual)
    );
}

fn assert_ok_or_warn(reply, label) {
    let actual = extract_status_reply(reply, label);
    assert(
        actual == "OK" || actual == "WARN",
        label + ": 串口回复错误, expected=OK|WARN, actual=" + reply
    );
}

fn assert_warn_reply(reply, label) {
    assert_status_reply(reply, "WARN", label);
}

fn assert_led_pattern(l1, l2, l3, l4, l5, l6, l7, l8, label) {
    assert_eq(led_on(L1), l1, label + ": L1 状态错误");
    assert_eq(led_on(L2), l2, label + ": L2 状态错误");
    assert_eq(led_on(L3), l3, label + ": L3 状态错误");
    assert_eq(led_on(L4), l4, label + ": L4 状态错误");
    assert_eq(led_on(L5), l5, label + ": L5 状态错误");
    assert_eq(led_on(L6), l6, label + ": L6 状态错误");
    assert_eq(led_on(L7), l7, label + ": L7 状态错误");
    assert_eq(led_on(L8), l8, label + ": L8 状态错误");
}

fn assert_time_page(hour, minute, second_min, second_max, label) {
    let text = read_display();
    assert_time_text(text, hour, minute, second_min, second_max, label)
}

fn assert_time_text(text, hour, minute, second_min, second_max, label) {
    assert_regex(text, "^\\d{2}-\\d{2}-\\d{2}$", label + ": 时间格式错误");
    assert_eq(parse_int(text[0..2]), hour, label + ": 小时显示错误");
    assert_eq(parse_int(text[3..5]), minute, label + ": 分钟显示错误");
    let second = parse_int(text[6..8]);
    assert_int_window(second, second_min, second_max, label + ": 秒值");
    text
}

fn right_value_abs() {
    let value = display_number(5, 8);
    if value < 0 {
        0 - value
    } else {
        value
    }
}

fn assert_liquid_page(temp, value_min, value_max, label) {
    let text = display_text(30);
    assert_eq(display_number(1, 4), temp, label + ": 温度显示错误");
    let value = right_value_abs();
    assert_float_window_view(value, value_min, value_max, label + ": 液位值", text);
    text
}

fn assert_volume_page(spare_min, spare_max, volume_min, volume_max, label) {
    let text = display_text(30);
    let spare = display_number(1, 3);
    assert_int_window_view(spare, spare_min, spare_max, label + ": 剩余空间", text);
    let volume = right_value_abs();
    assert_float_window_view(volume, volume_min, volume_max, label + ": 体积值", text);
    text
}

fn assert_weight_page(temp, value_min, value_max, label) {
    let text = display_text(30);
    assert_eq(display_number(1, 4), temp, label + ": 温度显示错误");
    let value = display_number(5, 8);
    assert_float_window_view(value, value_min, value_max, label + ": 重量值", text);
    text
}

fn assert_exact_display(expected, label) {
    let text = display_text(30);
    assert_eq(text, expected, label + ": 显示内容错误");
    text
}

fn assert_uart_int_alarm(code, time_min, time_max, value_min, value_max, label) {
    let text = extract_bracket_packet(uart_take(), "[" + code + ",", label);
    assert(
        regex_is_match(text, "^\\[\\d{3},\\d{6},\\d+\\]$"),
        label + ": 串口格式错误, expected=[异常码,时间,整数值], actual=" + text
    );
    assert_eq(parse_int(text[1..4]), code, label + ": 异常码错误");
    let time_value = parse_int(text[5..11]);
    assert_int_window(time_value, time_min, time_max, label + ": 时间戳");
    let value = parse_int(text[12..(len(text) - 1)]);
    assert_int_window(value, value_min, value_max, label + ": 异常值");
    text
}

fn assert_uart_float_alarm(code, time_min, time_max, value_min, value_max, label) {
    let text = extract_bracket_packet(uart_take(), "[" + code + ",", label);
    assert(
        regex_is_match(text, "^\\[\\d{3},\\d{6},\\d+(\\.\\d{1,2})?\\]$"),
        label + ": 串口格式错误, expected=[异常码,时间,浮点值], actual=" + text
    );
    assert_eq(parse_int(text[1..4]), code, label + ": 异常码错误");
    let time_value = parse_int(text[5..11]);
    assert_int_window(time_value, time_min, time_max, label + ": 时间戳");
    let value = parse_float(text[12..(len(text) - 1)]);
    assert_float_window(value, value_min, value_max, label + ": 异常值");
    text
}

fn assert_uart_signed_int_alarm(code, time_min, time_max, value_abs_min, value_abs_max, label) {
    let text = extract_bracket_packet(uart_take(), "[" + code + ",", label);
    assert(
        regex_is_match(text, "^\\[\\d{3},\\d{6},-?\\d+\\]$"),
        label + ": 串口格式错误, expected=[异常码,时间,带符号整数值], actual=" + text
    );
    assert_eq(parse_int(text[1..4]), code, label + ": 异常码错误");
    let time_value = parse_int(text[5..11]);
    assert_int_window(time_value, time_min, time_max, label + ": 时间戳");
    let value = parse_int(text[12..(len(text) - 1)]);
    let value_abs = if value < 0 { 0 - value } else { value };
    assert_int_window(value_abs, value_abs_min, value_abs_max, label + ": 异常值绝对值");
    text
}

fn configure_rect_1_5_1_2() {
    send_uart("(L,1.5)(H,2.0)");
    let reply = uart_take();
    assert_status_reply(reply, "OK", "预置长方体尺寸");
}

fn configure_cylinder_small_params() {
    send_uart("(S,1)");
    let reply_s = uart_take();
    send_uart("(L,1.5)(H,2.0)");
    let reply_size = uart_take();
    assert_status_reply(reply_s, "OK", "切到圆柱体");
    assert_status_reply(reply_size, "OK", "预置圆柱体尺寸");
}

fn configure_cylinder_large_params() {
    send_uart("(S,1)");
    let reply_s = uart_take();
    send_uart("(H1,2.3)(H2,0.5)(L,3.0)(W,1.5)(H,2.5)");
    let reply_size = uart_take();
    send_uart("(r,2.0)");
    let reply_r = uart_take();
    assert_status_reply(reply_s, "OK", "切到圆柱体");
    assert_status_reply(reply_size, "OK", "预置大容器尺寸");
    assert_status_reply(reply_r, "OK", "预置圆柱体半径");
}

fn prepare_default_rect(temp, distance, voltage) {
    boot_clean();
    set_temperature_c(temp);
    set_distance_cm(distance);
    set_voltage(RB2, voltage);
    sensor_settle();
}

fn prepare_custom_calibration() {
    boot_clean();
    open_std_page(0);
    adjust_up(4);
    tap_key(S5, 80);
    settle();
    adjust_up(2);
    tap_times(S5, 2, 320);
    adjust_down(5);
    tap_key(S4, 80);
    settle();
}

fn prepare_rect_1_5_1_2(temp, distance) {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 2.3);
    sensor_settle();
    configure_rect_1_5_1_2();
    set_temperature_c(temp);
    set_distance_cm(distance);
    set_voltage(RB2, 2.3);
    sensor_settle();
}

fn prepare_cylinder_small(distance) {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    sensor_settle();
    configure_cylinder_small_params();
    set_temperature_c(25);
    set_distance_cm(distance);
    sensor_settle();
}

fn prepare_cylinder_large(distance, temp) {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 1.5);
    sensor_settle();
    configure_cylinder_large_params();
    set_temperature_c(temp);
    set_distance_cm(distance);
    set_voltage(RB2, 1.5);
    sensor_settle();
}

fn prepare_sphere_scene() {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 1.5);
    sensor_settle();
    send_uart("(S,0)");
    let reply_s = uart_take();
    set_temperature_c(76);
    set_distance_cm(34);
    set_voltage(RB2, 1.5);
    sensor_settle();
    assert_status_reply(reply_s, "OK", "切到球体");
}

fn prepare_504_alarm_300_scene() {
    set_frequency_hz(0);
    set_temperature_c(25);
    set_distance_cm(35);
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    ensure_sig_jumper();
    run_ms(20);
    set_rtc(3, 44, 5);
    run_ms(320);
    set_frequency_hz(300);
    run_ms(1000);
    set_frequency_hz(1700);
    run_ms(1000);
}

fn prepare_504_alarm_400_scene() {
    set_frequency_hz(0);
    set_temperature_c(25);
    set_distance_cm(35);
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    ensure_sig_jumper();
    run_ms(20);
    set_rtc(3, 44, 5);
    run_ms(320);
    set_frequency_hz(400);
    run_ms(1000);
    set_frequency_hz(1700);
    run_ms(1000);
}

fn prepare_504_residual_034405_scene() {
    set_temperature_c(25);
    set_distance_cm(35);
    boot_with_sig();
    set_rtc(3, 44, 5);
    settle();
}

fn prepare_504_residual_221842_scene() {
    set_temperature_c(25);
    set_distance_cm(35);
    boot_with_sig();
    set_rtc(22, 18, 42);
    settle();
}

fn prepare_504_residual_2200_scene() {
    prepare_504_residual_221842_scene();
    set_frequency_hz(2200);
    run_ms(2100);
}

fn prepare_502_alarm_scene() {
    set_frequency_hz(0);
    set_temperature_c(25);
    set_distance_cm(35);
    reset(POWER_RESET);
    run_ms(320);
    ensure_sig_jumper();
    run_ms(5);
    set_rtc(22, 18, 42);
    run_ms(320);
    set_frequency_hz(2200);
    run_ms(2100);
}

fn prepare_503_alarm_scene() {
    set_frequency_hz(0);
    set_temperature_c(25);
    set_distance_cm(35);
    ensure_sig_jumper();
    sensor_settle();
    set_rtc(23, 33, 44);
    run_ms(320);
    set_frequency_hz(1500);
    run_ms(1200);
    set_frequency_hz(1300);
    run_ms(1200);
    set_frequency_hz(1000);
    run_ms(1200);
    set_frequency_hz(800);
    run_ms(1200);
    set_frequency_hz(500);
    run_ms(1200);
}

fn prepare_row53_scene() {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    sensor_settle();
    send_uart("(S,1)");
    let reply_s = uart_take();
    send_uart("(L,1.5)(H,2.0)(H2,0.5)");
    let reply_size = uart_take();
    set_rtc(17, 45, 23);
    settle();
    set_temperature_c(25);
    set_distance_cm(10);
    sensor_settle();
    set_temperature_c(25);
    set_distance_cm(170);
    sensor_settle();
    assert_status_reply(reply_s, "OK", "切到圆柱体");
    assert_status_reply(reply_size, "OK", "预置低液位场景参数");
}

fn prepare_row59_scene() {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 1.5);
    sensor_settle();
    configure_cylinder_large_params();
    set_rtc(17, 45, 23);
    settle();
    set_temperature_c(25);
    set_distance_cm(5);
    set_voltage(RB2, 1.5);
    sensor_settle();
}

ckpt(1, "时间显示", "L1点亮, 其余指示灯熄灭", || {
    boot_with_sig();
    set_rtc(23, 59, 50);
    settle();
    assert_led_pattern(true, false, false, false, false, false, false, false, "上电指示灯");
    led_summary()
});

ckpt(2, "初始状态", "23-59-50", || {
    assert_time_page(23, 59, 50, 50, "默认时间页")
});

ckpt(3, "无异常状态发生", "继电器断开", || {
    assert_bool_state(relay_on(), false, "无异常时继电器");
    relay_summary()
});

ckpt(4, "2s内频率在300hz->1700hz", "L8以0.2s为间隔闪烁", || {
    prepare_504_alarm_300_scene();
    let stats = watch_led_stats(L8, 3000);
    assert_int_window(stats.changes, 14, 16, "504 场景 L8 闪烁次数");
    "changes=" + stats.changes
});

ckpt(5, "504异常状态发生", "继电器吸合(无异常断开)", || {
    prepare_504_alarm_300_scene();
    watch_led_stats(L8, 1000);
    let relay_now = relay_on();
    assert_bool_state(relay_now, true, "504 异常后继电器");
    "relay=" + bool_text(relay_now)
});

ckpt(6, "时间:03:44:05,频率值:400hz->1700hz", "串口返回 [504,034405,1300]", || {
    prepare_504_alarm_400_scene();
    assert_uart_signed_int_alarm(504, 34405, 34408, 1000, 1400, "504 串口上报")
});

ckpt(7, "初始状态", "03-44-05", || {
    prepare_504_residual_034405_scene();
    assert_time_page(3, 44, 5, 5, "03:44:05 时间页")
});

ckpt(8, "无异常状态发生", "L1点亮, 其余指示灯熄灭", || {
    assert_led_pattern(true, false, false, false, false, false, false, false, "03:44:05 页面指示灯");
    led_summary()
});

ckpt(9, "时间:22:18:42", "22-18-42", || {
    prepare_504_residual_221842_scene();
    assert_time_page(22, 18, 42, 42, "22:18:42 时间页")
});

ckpt(10, "无异常状态发生", "继电器断开", || {
    assert_bool_state(relay_on(), false, "22:18:42 无异常时继电器");
    relay_summary()
});

ckpt(11, "频率值:2200", "L1, L6点亮, 其余指示灯熄灭", || {
    set_frequency_hz(2200);
    run_ms(2100);
    assert_led_pattern(true, false, false, false, false, true, false, false, "502 异常指示灯");
    led_summary()
});

ckpt(12, "22:18:42, 频率值:2200", "串口返回 [502,221842,2200]", || {
    assert_uart_int_alarm(502, 221842, 221843, 2134, 2266, "502 串口上报")
});

ckpt(13, "502异常状态发生", "继电器吸合", || {
    assert_bool_state(relay_on(), true, "502 异常后继电器");
    relay_summary()
});

ckpt(14, "切换8个子参数界面", "P1-   1.0, P2-   0.1, P3- 2000, P4-    2, P5-   1.0, P6-   1.0, P7-   1.0, P8-   1.0", || {
    boot_clean();
    open_param_page(1);
    let p1 = read_display();
    tap_key(S5, 80);
    settle();
    let p2 = read_display();
    tap_key(S5, 80);
    settle();
    let p3 = read_display();
    tap_key(S5, 80);
    settle();
    let p4 = read_display();
    tap_key(S5, 80);
    settle();
    let p5 = read_display();
    tap_key(S5, 80);
    settle();
    let p6 = read_display();
    tap_key(S5, 80);
    settle();
    let p7 = read_display();
    tap_key(S5, 80);
    settle();
    let p8 = read_display();
    assert_eq(p1, "P1-   1.0", "参数页 P1: 显示内容错误");
    assert_eq(p2, "P2-   0.1", "参数页 P2: 显示内容错误");
    assert_eq(p3, "P3- 2000", "参数页 P3: 显示内容错误");
    assert_eq(p4, "P4-    2", "参数页 P4: 显示内容错误");
    assert_eq(p5, "P5-   1.0", "参数页 P5: 显示内容错误");
    assert_eq(p6, "P6-   1.0", "参数页 P6: 显示内容错误");
    assert_eq(p7, "P7-   1.0", "参数页 P7: 显示内容错误");
    assert_eq(p8, "P8-   1.0", "参数页 P8: 显示内容错误");
    p1 + "," + p2 + "," + p3 + "," + p4 + "," + p5 + "," + p6 + "," + p7 + "," + p8
});

ckpt(15, "切换标定界面", "E00   0.0", || {
    tap_key(S4, 80);
    settle();
    assert_exact_display("E00   0.0", "空载标定点")
});

ckpt(16, "0吨标定点加0.2", "E00   0.2", || {
    adjust_up(2);
    assert_exact_display("E00   0.2", "空载标定加 0.2V")
});

ckpt(17, "0吨标定点减0.1", "E00   0.1", || {
    adjust_down(1);
    assert_exact_display("E00   0.1", "空载标定减 0.1V")
});

ckpt(18, "5吨标定点", "E05   1.0", || {
    next_page();
    assert_exact_display("E05   1.0", "5 吨标定点")
});

ckpt(19, "10吨标定点减0.2", "E10   1.8", || {
    next_page();
    adjust_down(2);
    assert_exact_display("E10   1.8", "10 吨标定点减 0.2V")
});

ckpt(20, "10吨标定点加0.1", "E10   1.9", || {
    adjust_up(1);
    assert_exact_display("E10   1.9", "10 吨标定点加 0.1V")
});

ckpt(21, "15吨标定点4.0", "E15   4.0", || {
    next_page();
    assert_exact_display("E15   4.0", "15 吨标定点")
});

ckpt(22, "20吨标定点加0.1", "E20   5.1", || {
    next_page();
    adjust_up(1);
    assert_exact_display("E20   5.1", "20 吨标定点加 0.1V")
});

ckpt(23, "0吨标定点减0.1", "E00   0.0", || {
    next_page();
    adjust_down(1);
    assert_exact_display("E00   0.0", "空载标定下限保护")
});

ckpt(24, "距离值:35cm, 液位界面(长方体, L:1, W:1, H:1)", "25-0.64", || {
    prepare_default_rect(25, 35, 2.3);
    open_data_page(1);
    assert_liquid_page(25, 0.63, 0.66, "35cm 液位页")
});

ckpt(25, "距离值:35cm, 体积显示(长方体, L:1, W:1, H:1)", "36- 0.6", || {
    next_page();
    assert_volume_page(34, 37, 0.5, 0.7, "35cm 体积页")
});

ckpt(26, "距离值:35cm, 温度值:25, VRB2:2.3V, 重量显示(长方体, L:1, W:1, H:1)", "25- 10.7", || {
    next_page();
    assert_weight_page(25, 10.6, 10.8, "25C 2.3V 重量页")
});

ckpt(27, "距离值:58cm, 温度值:48, 液位界面(长方体, L:1, W:1, H:1)", "48-0.42", || {
    prepare_default_rect(48, 58, 2.3);
    open_data_page(1);
    assert_liquid_page(48, 0.37, 0.44, "58cm 液位页")
});

ckpt(28, "距离值:58cm, 体积显示(长方体, L:1, W:1, H:1)", "61- 0.4", || {
    next_page();
    assert_volume_page(60, 61, 0.35, 0.45, "58cm 体积页")
});

ckpt(29, "温度值:48, VRB2:2.3V, 重量显示(长方体, L:1, W:1, H:1)", "48-10.3", || {
    next_page();
    assert_weight_page(48, 10.2, 10.4, "48C 2.3V 重量页")
});

ckpt(30, "距离值:86cm, 温度值:78, 液位界面(长方体, L:1, W:1, H:1)", "78-0.05", || {
    prepare_default_rect(78, 86, 3.5);
    open_data_page(1);
    assert_liquid_page(78, 0.04, 0.06, "86cm 液位页")
});

ckpt(31, "距离值:86cm, 体积显示(长方体, L:1, W:1, H:1)", "95- 0.1", || {
    next_page();
    assert_volume_page(94, 96, 0.0, 0.1, "86cm 体积页")
});

ckpt(32, "温度值:78, VRB2:3.5V, 重量显示(长方体, L:1, W:1, H:1)", "78- 12.9", || {
    next_page();
    assert_weight_page(78, 12.8, 13.0, "78C 3.5V 重量页")
});

ckpt(33, "距离值:65cm, 温度值:-17, 液位界面(长方体, L:1, W:1, H:1)", "-17-0.38", || {
    prepare_default_rect(-17, 65, 0.0);
    open_data_page(1);
    assert_liquid_page(-17, 0.37, 0.40, "-17C 液位页")
});

ckpt(34, "距离值:65cm, 体积显示(长方体, L:1, W:1, H:1)", "61- 0.4", || {
    next_page();
    assert_volume_page(60, 62, 0.3, 0.4, "-17C 体积页")
});

ckpt(35, "温度值:-17, VRB2:0V, 重量显示(长方体, L:1, W:1, H:1)", "-17- 2.0", || {
    next_page();
    assert_weight_page(-17, 1.8, 2.1, "-17C 0V 重量页")
});

ckpt(36, "距离值:35cm, 温度值:-38, 液位界面(长方体, L:1, W:1, H:1)", "-38-0.68", || {
    prepare_default_rect(-38, 35, 4.5);
    open_data_page(1);
    assert_liquid_page(-38, 0.67, 0.69, "-38C 液位页")
});

ckpt(37, "距离值:35cm, 体积显示(长方体, L:1, W:1, H:1)", "32- 0.7", || {
    next_page();
    assert_volume_page(31, 33, 0.6, 0.7, "-38C 体积页")
});

ckpt(38, "温度值:-38, VRB4.5V, 重量显示(长方体, L:1, W:1, H:1)", "-38- 20.0", || {
    next_page();
    assert_weight_page(-38, 19.8, 20.0, "-38C 4.5V 重量页")
});

ckpt(39, "0吨标定点加0.4, 5吨标定点加0.2, 15吨标定点减1.5", "E00   0.4 -> E05   1.2 -> E15   3.5", || {
    boot_clean();
    open_std_page(0);
    adjust_up(4);
    let e00 = read_display();
    tap_key(S5, 80);
    settle();
    adjust_up(2);
    let e05 = read_display();
    tap_times(S5, 2, 320);
    adjust_down(5);
    let e15 = read_display();
    assert_eq(e00, "E00   0.4", "标定点 E00: 显示内容错误");
    assert_eq(e05, "E05   1.2", "标定点 E05: 显示内容错误");
    assert_eq(e15, "E15   3.5", "标定点 E15: 显示内容错误");
    tap_key(S4, 80);
    settle();
    e00 + "," + e05 + "," + e15
});

ckpt(40, "VRB2:0.8V, 重量显示", "-38- 6.6", || {
    set_temperature_c(-38);
    set_voltage(RB2, 0.8);
    sensor_settle();
    open_data_page(3);
    assert_weight_page(-38, 6.3, 6.7, "自定义标定 0.8V 重量页")
});

ckpt(41, "VRB2:2.5V, 温度值:28, 重量显示", "28-11.5", || {
    set_temperature_c(28);
    set_voltage(RB2, 2.5);
    sensor_settle();
    assert_weight_page(28, 11.4, 11.6, "自定义标定 2.5V 重量页")
});

ckpt(42, "VRB2:4V, 温度值:28, 重量显示", "28-16.5", || {
    set_temperature_c(28);
    set_voltage(RB2, 4.0);
    sensor_settle();
    assert_weight_page(28, 16.4, 16.6, "自定义标定 4.0V 重量页")
});

ckpt(43, "发送(F,?)", "串口返回:(F,2000)", || {
    boot_clean();
    assert_query("(F,?)", "(F,2000)", "查询频率参数")
});

ckpt(44, "发送(F,1500)", "串口返回:OK", || {
    send_uart("(F,1500)");
    let text = uart_take();
    assert_eq(text, "OK", "设置频率参数: 串口回复错误");
    text
});

ckpt(45, "频率值:1300->1800", "串口返回:[502,132454,1800]", || {
    ensure_sig_jumper();
    run_ms(5);
    set_rtc(13, 24, 54);
    run_ms(320);
    set_frequency_hz(1300);
    run_ms(1200);
    set_frequency_hz(1800);
    run_ms(1200);
    assert_uart_int_alarm(502, 132454, 132457, 1500, 1900, "1300->1800 数值异常")
});

ckpt(46, "发送(H1,?)", "串口返回:(H1,1.0)", || {
    boot_clean();
    assert_query("(H1,?)", "(H1,1.0)", "查询 H1")
});

ckpt(47, "发送(H1,1.8)(H2,0.5) -> (L,1.5)(H,2.0)", "串口返回:OK -> OK", || {
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 2.3);
    sensor_settle();
    send_uart("(H1,1.8)(H2,0.5)");
    let first = uart_take();
    send_uart("(L,1.5)(H,2.0)");
    let second = uart_take();
    assert_regex(first, "^OK", "设置 H1/H2: 串口回复错误");
    assert_regex(second, "^OK", "设置 L/H: 串口回复错误");
    first + "," + second
});

ckpt(48, "距离值:76cm, 液位界面(长方体, L:1.5, W:1, H:2)", "32-1.21", || {
    set_temperature_c(32);
    set_distance_cm(76);
    set_voltage(RB2, 2.3);
    sensor_settle();
    open_data_page(1);
    assert_liquid_page(32, 1.18, 1.22, "1.5x1x2 液位页")
});

ckpt(49, "距离值:76cm, 体积显示(长方体, L:1.5, W:1, H:2)", "38- 1.8", || {
    next_page();
    assert_volume_page(38, 39, 1.7, 1.9, "1.5x1x2 体积页")
});

ckpt(50, "发送(S,?)", "串口返回:(S,2)", || {
    boot_clean();
    assert_query("(S,?)", "(S,2)", "查询容器类型")
});

ckpt(51, "切换圆柱体, 距离值:76cm, 体积显示(圆柱体, L:1.5, W:1, H:2)", "39- 3.9", || {
    set_temperature_c(25);
    set_distance_cm(35);
    sensor_settle();
    configure_cylinder_small_params();
    set_distance_cm(76);
    sensor_settle();
    open_data_page(2);
    assert_volume_page(38, 40, 3.8, 4.1, "圆柱体 76cm 体积页")
});

ckpt(52, "距离值:34cm, 体积显示(圆柱体, L:1.5, H:2)", "17- 5.2", || {
    set_distance_cm(34);
    sensor_settle();
    assert_volume_page(16, 18, 5.0, 5.4, "圆柱体 34cm 体积页")
});

ckpt(53, "距离值:170cm", "串口返回:[501,174523,0.3]", || {
    prepare_row53_scene();
    open_data_page(1);
    assert_uart_float_alarm(501, 174523, 174524, 0.25, 0.31, "低液位上报")
});

ckpt(54, "距离值:170cm", "继电器吸合(无异常时断开)", || {
    assert_bool_state(relay_on(), true, "低液位异常时继电器");
    relay_summary()
});

ckpt(55, "发送(H1,2.3)(H2,0.5)(S,3)(L,3.0)(W,1.5)(H,2.5)", "串口返回:WARN", || {
    boot_clean();
    set_temperature_c(25);
    set_distance_cm(35);
    set_voltage(RB2, 2.3);
    sensor_settle();
    send_uart("(S,1)");
    let switch_reply = uart_take();
    send_uart("(H1,2.3)(H2,0.5)(S,3)(L,3.0)(W,1.5)(H,2.5)");
    let text = uart_take();
    assert_regex(switch_reply, "^OK", "先切到圆柱体: 串口回复错误");
    assert_warn_reply(text, "部分生效参数返回值");
    text
});

ckpt(56, "串口发送(r,2.0), 距离值:87cm, 体积显示(圆柱体, r:2, L:3, W:1.5, H:2.5)", "35- 20.4", || {
    send_uart("(r,2.0)");
    let reply_r = uart_take();
    set_temperature_c(25);
    set_distance_cm(87);
    set_voltage(RB2, 1.5);
    sensor_settle();
    open_data_page(2);
    assert_status_reply(reply_r, "OK", "设置 r=2.0");
    assert_volume_page(34, 36, 20.2, 20.7, "圆柱体大容器 87cm 体积页")
});

ckpt(57, "距离值:87cm, 液位显示(圆柱体, r:2, L:3, W:1.5, H:2.5)", "21- 1.6", || {
    set_temperature_c(21);
    set_distance_cm(87);
    set_voltage(RB2, 1.5);
    sensor_settle();
    tap_times(S5, 3, 160);
    assert_liquid_page(21, 1.58, 1.63, "圆柱体大容器 87cm 液位页")
});

ckpt(58, "距离值:45cm, 体积显示(圆柱体, r:2, L:3, W:1.5, H:2.5)", "18- 25.7", || {
    set_distance_cm(45);
    sensor_settle();
    next_page();
    assert_volume_page(18, 19, 25.5, 25.9, "圆柱体大容器 45cm 体积页")
});

ckpt(59, "距离值:5cm", "串口返回:[500,174523,2.4]", || {
    prepare_row59_scene();
    open_data_page(1);
    assert_uart_float_alarm(500, 174523, 174524, 2.40, 2.45, "高液位上报")
});

ckpt(60, "发送(T:12:13:15)", "串口返回:OK, 数码管显示 12-13-15", || {
    boot_clean();
    send_uart("(T:121315)");
    let reply = uart_take();
    let text = read_display();
    assert_eq(reply, "OK", "设置 12:13:15: 串口回复错误");
    assert_time_text(text, 12, 13, 15, 15, "12:13:15 时间页");
    reply + "," + text
});

ckpt(61, "(T:23:33:44)", "串口返回:OK, 数码管显示 23-33-44", || {
    send_uart("(T:233344)");
    let reply = uart_take();
    let text = read_display();
    assert_eq(reply, "OK", "设置 23:33:44: 串口回复错误");
    assert_time_text(text, 23, 33, 44, 44, "23:33:44 时间页");
    reply + "," + text
});

ckpt(62, "频率趋势异常", "L7以0.1s为间隔闪烁", || {
    prepare_503_alarm_scene();
    let stats = watch_led_stats(L7, 1000);
    assert_int_window(stats.changes, 9, 11, "503 场景 L7 闪烁次数");
    "changes=" + stats.changes
});

ckpt(63, "5s内, 频率值:1500->1300->1000->800->500", "串口返回:[503,233344,0]", || {
    let text = uart_take();
    assert_regex(text, "^\\[503,23334[4-9],0\\]$", "503 串口上报: 格式错误");
    text
});

ckpt(64, "发送(T:28:63:44)", "串口返回:ERROR, 数码管显示 23-33-44", || {
    boot_clean();
    send_uart("(T:233344)");
    let first = uart_take();
    send_uart("(T:286344)");
    let reply = uart_take();
    let text = read_display();
    assert_status_reply(first, "OK", "先设置合法时间: 串口回复错误");
    assert_status_reply(reply, "ERROR", "非法时间设置: 串口回复错误");
    assert_time_text(text, 23, 33, 44, 44, "非法时间后保持原显示");
    reply + "," + text
});

ckpt(65, "发送(S,4) -> (r,1.0)(S,3) -> (S,?)", "串口返回:ERROR -> WARN -> (S,1)", || {
    boot_clean();
    send_uart("(S,1)");
    let init = uart_take();
    send_uart("(S,4)");
    let first = uart_take();
    send_uart("(r,1.0)(S,3)");
    let second = uart_take();
    let third = query_uart("(S,?)");
    assert_eq(init, "OK", "先切到圆柱体: 串口回复错误");
    assert_eq(first, "ERROR", "非法容器类型设置: 串口回复错误");
    assert_eq(second, "WARN", "部分生效参数: 串口回复错误");
    assert_eq(third, "(S,1)", "查询容器类型: 返回值错误");
    first + "," + second + "," + third
});

ckpt(66, "液位显示, 发送(S,0), 温度值:76, VR37:1.5V, 距离值:34cm", "76-1.67", || {
    send_uart("(S,0)");
    let reply_s = uart_take();
    set_temperature_c(76);
    set_distance_cm(34);
    set_voltage(RB2, 1.5);
    sensor_settle();
    assert_status_reply(reply_s, "OK", "切到球体");
    open_data_page(1);
    assert_liquid_page(76, 1.60, 1.68, "球体 34cm 液位页")
});

ckpt(67, "体积显示, 发送(S,0), 温度值:76, VR37:1.5V, 距离值:34cm", "9- 3.8", || {
    next_page();
    assert_volume_page(9, 11, 3.6, 3.9, "球体 34cm 体积页")
});
