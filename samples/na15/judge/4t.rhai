key_mode(KEYBOARD);

fn settle() {
    run_ms(120);
}

fn page_settle() {
    run_ms(30);
}

fn sensor_settle() {
    run_ms(320);
}

fn freq_settle() {
    run_ms(1200);
}

fn read_display() {
    display_text(30)
}

fn read_num(start, end) {
    display_number(start, end, 30)
}

fn key_response_ns() {
    100_000_000
}

fn seg_response_ns() {
    100_000_000
}

fn key_to_seg_response_ns() {
    key_response_ns() + seg_response_ns()
}

fn uart_response_ns() {
    500_000_000
}

fn avoid_response_ns() {
    500_000_000
}

fn min_int(a, b) {
    if a < b {
        a
    } else {
        b
    }
}

fn max_int(a, b) {
    if a > b {
        a
    } else {
        b
    }
}

fn min_float(a, b) {
    if a < b {
        a
    } else {
        b
    }
}

fn max_float(a, b) {
    if a > b {
        a
    } else {
        b
    }
}

fn clamp_float(value, low, high) {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

fn raw_speed_cm_s(r_value, b_value, freq_hz) {
    let speed = 3.14 * r_value * freq_hz.to_float() / 100.0 + b_value;
    if speed < 0.0 {
        0.0
    } else {
        speed
    }
}

fn speed_formula_window(r_value, b_value, freq_hz) {
    let freq_f = freq_hz.to_float();
    let raw_min = raw_speed_cm_s(r_value, b_value, freq_f * 0.92);
    let raw_max = raw_speed_cm_s(r_value, b_value, freq_f * 1.08);
    #{
        raw_min: raw_min,
        raw_max: raw_max,
        display_min: max_float(0.0, raw_min - 0.05),
        display_max: raw_max + 0.05,
    }
}

fn distance_formula_window(distance_cm) {
    #{
        min: max_int(0, distance_cm - 3),
        max: distance_cm + 3,
    }
}

fn floor_int(value) {
    let int_part = value.to_int();
    if value < 0.0 && int_part.to_float() != value {
        int_part - 1
    } else {
        int_part
    }
}

fn ceil_int(value) {
    let int_part = value.to_int();
    if value > 0.0 && int_part.to_float() != value {
        int_part + 1
    } else {
        int_part
    }
}

fn speed_segment(freq_hz, duration_ms) {
    #{
        freq_hz: freq_hz,
        duration_ms: duration_ms,
    }
}

fn bool_text(value) {
    if value {
        "on"
    } else {
        "off"
    }
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

fn led_snapshot() {
    #{
        l1: led_on(L1),
        l2: led_on(L2),
        l3: led_on(L3),
        l4: led_on(L4),
        l5: led_on(L5),
        l6: led_on(L6),
        l7: led_on(L7),
        l8: led_on(L8),
    }
}

fn led_snapshot_summary(snapshot) {
    "L1=" + bool_text(snapshot.l1)
        + ",L2=" + bool_text(snapshot.l2)
        + ",L3=" + bool_text(snapshot.l3)
        + ",L4=" + bool_text(snapshot.l4)
        + ",L5=" + bool_text(snapshot.l5)
        + ",L6=" + bool_text(snapshot.l6)
        + ",L7=" + bool_text(snapshot.l7)
        + ",L8=" + bool_text(snapshot.l8)
}

fn relay_summary() {
    "relay=" + bool_text(relay_on())
}

fn ensure_sig_jumper() {
    if !jumper_installed(NET_SIG, SIG_OUT) {
        jumper_on(NET_SIG, SIG_OUT);
    }
}

fn press_key_expect_display_change(key, label) {
    let press_ns = sim_time_ns();
    set_key(key, true);
    let event = run_to_event("seg.change", key_to_seg_response_ns());
    if sim_time_ns() < press_ns + 80_000_000 {
        run_to_ns(press_ns + 80_000_000);
    }
    set_key(key, false);
    run_ms(30);
    assert_int_window(event.elapsed_ns, 0, key_to_seg_response_ns(), label + ": 按键到数码管响应超时");
    event.elapsed_ns
}

fn press_s4_expect_running(label) {
    let press_ns = sim_time_ns();
    set_key(S4, true);
    let elapsed_ns = run_to(|| relay_on() && led_on(L1), key_response_ns());
    if sim_time_ns() < press_ns + 80_000_000 {
        run_to_ns(press_ns + 80_000_000);
    }
    set_key(S4, false);
    run_ms(30);
    assert_int_window(elapsed_ns, 0, key_response_ns(), label + ": 运行状态响应超时");
    #{
        press_ns: press_ns,
        elapsed_ns: elapsed_ns,
    }
}

fn press_s4_expect_relay_off_detail(label) {
    let press_ns = sim_time_ns();
    set_key(S4, true);
    let elapsed_ns = run_to(|| !relay_on(), key_response_ns());
    if sim_time_ns() < press_ns + 80_000_000 {
        run_to_ns(press_ns + 80_000_000);
    }
    set_key(S4, false);
    run_ms(30);
    assert_int_window(elapsed_ns, 0, key_response_ns(), label + ": 继电器断开超时");
    #{
        press_ns: press_ns,
        elapsed_ns: elapsed_ns,
    }
}

fn set_voltage_expect_led2(voltage_v, expected_on, label) {
    let before = led_on(L2);
    set_voltage(AIN1, voltage_v);
    let elapsed_ns = run_to(|| led_on(L2) == expected_on, key_response_ns());
    assert(before != expected_on, label + ": 前置状态未建立");
    assert_int_window(elapsed_ns, 0, key_response_ns(), label + ": L2 响应超时");
    run_ms(30);
    elapsed_ns
}

fn trigger_obstacle_expect_wait(distance_cm, label) {
    set_distance_cm(distance_cm);
    let elapsed_ns = run_to(|| !relay_on(), avoid_response_ns());
    assert_int_window(elapsed_ns, 0, avoid_response_ns(), label + ": 避障切换等待超时");
    run_ms(30);
    elapsed_ns
}

fn set_distance_expect_wait_page(distance_cm, label) {
    set_distance_cm(distance_cm);
    let event = run_to_event("seg.change", avoid_response_ns());
    assert_int_window(event.elapsed_ns, 0, avoid_response_ns(), label + ": 距离显示响应超时");
    run_ms(30);
    assert_distance_formula_page(distance_cm, label)
}

fn boot_clean() {
    reset(POWER_RESET);
    key_mode(KEYBOARD);
    run_ms(20);
}

fn boot_scene(freq_hz, distance_cm, voltage_v) {
    boot_clean();
    ensure_sig_jumper();
    set_frequency_hz(freq_hz);
    set_distance_cm(distance_cm);
    set_voltage(AIN1, voltage_v);
    sensor_settle();
}

fn tap_times(key, count, delay_ms) {
    let i = 0;
    while i < count {
        press_key_expect_display_change(key, "按键 S" + key);
        i += 1;
    }
    run_ms(delay_ms);
}

fn next_page() {
    press_key_expect_display_change(S8, "界面切换");
    page_settle();
}

fn select_param() {
    tap_key(S9, 80);
    page_settle();
}

fn reset_position() {
    press_key_expect_display_change(S5, "重置按键");
    page_settle();
}

fn adjust_up(steps) {
    tap_times(S12, steps, 320);
}

fn adjust_down(steps) {
    tap_times(S13, steps, 320);
}

fn run_to_elapsed_from(press_ns, elapsed_ms) {
    let target_ns = press_ns + elapsed_ms * 1_000_000;
    if sim_time_ns() < target_ns {
        run_to_ns(target_ns);
    }
}

fn send_uart_raw(text) {
    uart_take();
    uart_write(text);
}

fn send_uart_expect(text, expected, label) {
    send_uart_raw(text);
    let elapsed_ns = run_to(|| uart_peek(10) == expected, uart_response_ns());
    let reply = uart_take(10);
    assert_int_window(elapsed_ns, 0, uart_response_ns(), label + ": 串口响应超时");
    assert_eq(reply, expected, label + ": 串口回复错误");
    reply
}

fn query_uart(text, expected, label) {
    send_uart_expect(text, expected, label)
}

fn query_uart_point_detail(label) {
    send_uart_raw("#");
    let elapsed_ns = run_to(
        || regex_is_match(uart_peek(10), "^\\(\\d{1,3},\\d{1,3}\\)$"),
        uart_response_ns()
    );
    let reply = uart_take(10);
    assert_int_window(elapsed_ns, 0, uart_response_ns(), label + ": 串口响应超时");
    assert_regex(reply, "^\\(\\d{1,3},\\d{1,3}\\)$", label + ": 串口坐标格式错误");
    #{
        reply: reply,
        elapsed_ns: elapsed_ns,
        point: parse_point(reply, label),
    }
}

fn query_uart_point(label) {
    query_uart_point_detail(label).reply
}

fn set_destination(x, y, label) {
    let cmd = "(" + x + "," + y + ")";
    send_uart_expect(cmd, "Got it", label)
}

fn assert_int_window(actual, min, max, label) {
    assert_in(actual, min..=max, label + ": 数值超出范围");
}

fn assert_float_window(actual, min, max, label) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际=" + actual
    );
}

fn assert_int_window_view(actual, min, max, label, view) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际=" + actual + ",显示=" + view
    );
}

fn assert_float_window_view(actual, min, max, label, view) {
    assert(
        actual >= min && actual <= max,
        label + ": 应在[" + min + "," + max + "], 实际=" + actual + ",显示=" + view
    );
}

fn assert_bool_state(actual, expected, label) {
    assert(
        actual == expected,
        label + ": 状态错误, expected=" + bool_text(expected) + ", actual=" + bool_text(actual)
    );
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

fn assert_led_snapshot_pattern(snapshot, l1, l2, l3, l4, l5, l6, l7, l8, label) {
    assert_eq(snapshot.l1, l1, label + ": L1 状态错误");
    assert_eq(snapshot.l2, l2, label + ": L2 状态错误");
    assert_eq(snapshot.l3, l3, label + ": L3 状态错误");
    assert_eq(snapshot.l4, l4, label + ": L4 状态错误");
    assert_eq(snapshot.l5, l5, label + ": L5 状态错误");
    assert_eq(snapshot.l6, l6, label + ": L6 状态错误");
    assert_eq(snapshot.l7, l7, label + ": L7 状态错误");
    assert_eq(snapshot.l8, l8, label + ": L8 状态错误");
}

fn assert_coord_page_exact(x, y, label) {
    let text = read_display();
    assert_regex(text, "^L", label + ": 坐标页前缀错误");
    assert_eq(read_num(2, 4), x, label + ": X 坐标显示错误");
    assert_eq(read_num(6, 8), y, label + ": Y 坐标显示错误");
    text
}

fn assert_speed_idle_page(label) {
    let text = read_display();
    assert_regex(text, "^E2\\s*-{5}$", label + ": 空闲速度页格式错误");
    text
}

fn assert_speed_run_page(min, max, label) {
    let text = read_display();
    assert_regex(text, "^E1", label + ": 运行速度页前缀错误");
    let value = read_num(4, 8);
    assert_float_window_view(value, min, max, label + ": 速度值", text);
    text
}

fn assert_speed_formula_page(r_value, b_value, freq_hz, label) {
    let window = speed_formula_window(r_value, b_value, freq_hz);
    assert_speed_run_page(window.display_min, window.display_max, label)
}

fn assert_speed_wait_page(min, max, label) {
    let text = read_display();
    assert_regex(text, "^E3", label + ": 等待速度页前缀错误");
    let value = read_num(4, 8);
    assert_int_window_view(value, min, max, label + ": 障碍物距离", text);
    text
}

fn assert_distance_formula_page(distance_cm, label) {
    let window = distance_formula_window(distance_cm);
    assert_speed_wait_page(window.min, window.max, label)
}

fn assert_param_exact(r_value, b_value, label) {
    let text = read_display();
    assert_regex(text, "^P", label + ": 参数页前缀错误");
    assert_float_window_view(read_num(3, 4), r_value, r_value, label + ": R 参数", text);
    assert_int_window_view(read_num(6, 8), b_value, b_value, label + ": B 参数", text);
    text
}

fn parse_point(reply, label) {
    assert_regex(reply, "^\\(\\d{1,3},\\d{1,3}\\)$", label + ": 串口坐标格式错误");
    let comma = reply.index_of(",");
    assert(comma >= 0, label + ": 坐标缺少逗号, actual=" + reply);
    #{
        x: parse_int(reply[1..comma]),
        y: parse_int(reply[(comma + 1)..(len(reply) - 1)]),
    }
}

fn assert_position_query_exact(x, y, label) {
    let reply = query_uart_point(label);
    let point = parse_point(reply, label);
    assert_eq(point.x, x, label + ": X 坐标错误");
    assert_eq(point.y, y, label + ": Y 坐标错误");
    reply
}

fn assert_position_query_window(x_min, x_max, y_min, y_max, label) {
    let reply = query_uart_point(label);
    let point = parse_point(reply, label);
    assert_int_window(point.x, x_min, x_max, label + ": X 坐标");
    assert_int_window(point.y, y_min, y_max, label + ": Y 坐标");
    reply
}

fn speed_segments_duration_ms(segments) {
    let total_ms = 0;
    let i = 0;
    while i < len(segments) {
        total_ms += segments[i].duration_ms;
        i += 1;
    }
    total_ms
}

fn speed_segments_distance_window(r_value, b_value, segments, start_elapsed_ns, tail_extra_ns) {
    let start_delay_ms = start_elapsed_ns.to_float() / 1_000_000.0;
    let tail_extra_ms = tail_extra_ns.to_float() / 1_000_000.0;
    let min_cm = 0.0;
    let max_cm = 0.0;
    let last = len(segments) - 1;
    let i = 0;
    while i < len(segments) {
        let seg = segments[i];
        let duration_ms = seg.duration_ms.to_float();
        let window = speed_formula_window(r_value, b_value, seg.freq_hz);
        let min_ms = duration_ms;
        let max_ms = duration_ms;
        if i == 0 {
            min_ms = max_float(0.0, duration_ms - start_delay_ms);
        }
        if i == last {
            max_ms = duration_ms + tail_extra_ms;
        }
        min_cm += window.raw_min * min_ms / 1000.0;
        max_cm += window.raw_max * max_ms / 1000.0;
        i += 1;
    }
    #{
        min_cm: min_cm,
        max_cm: max_cm,
    }
}

fn assert_projected_point_window(point, x0, y0, dx, dy, path_cm, movement, label) {
    let dist_min = clamp_float(movement.min_cm, 0.0, path_cm);
    let dist_max = clamp_float(movement.max_cm, 0.0, path_cm);
    let x0_f = x0.to_float();
    let y0_f = y0.to_float();
    let dx_f = dx.to_float();
    let dy_f = dy.to_float();
    let x_min_f = x0_f + dx_f * dist_min / path_cm;
    let x_max_f = x0_f + dx_f * dist_max / path_cm;
    let y_min_f = y0_f + dy_f * dist_min / path_cm;
    let y_max_f = y0_f + dy_f * dist_max / path_cm;
    let x_low = floor_int(min_float(x_min_f, x_max_f)) - 1;
    let x_high = ceil_int(max_float(x_min_f, x_max_f)) + 1;
    let y_low = floor_int(min_float(y_min_f, y_max_f)) - 1;
    let y_high = ceil_int(max_float(y_min_f, y_max_f)) + 1;
    assert_int_window(point.x, x_low, x_high, label + ": X 坐标");
    assert_int_window(point.y, y_low, y_high, label + ": Y 坐标");
}

fn assert_projected_position_query_from_segments(start, x0, y0, dx, dy, path_cm, r_value, b_value, segments, label) {
    let elapsed_ms = speed_segments_duration_ms(segments);
    run_to_elapsed_from(start.press_ns, elapsed_ms);
    let detail = query_uart_point_detail(label);
    let movement = speed_segments_distance_window(
        r_value,
        b_value,
        segments,
        start.elapsed_ns,
        detail.elapsed_ns
    );
    assert_projected_point_window(detail.point, x0, y0, dx, dy, path_cm, movement, label);
    detail.reply
}

fn assert_projected_position_query_from_start(start, x0, y0, dx, dy, path_cm, r_value, b_value, freq_hz, elapsed_ms, label) {
    let segments = [speed_segment(freq_hz, elapsed_ms)];
    assert_projected_position_query_from_segments(
        start,
        x0,
        y0,
        dx,
        dy,
        path_cm,
        r_value,
        b_value,
        segments,
        label
    )
}

fn assert_projected_position_query_after_stop(scene, x0, y0, dx, dy, path_cm, r_value, b_value, segments, label) {
    let detail = query_uart_point_detail(label);
    let movement = speed_segments_distance_window(
        r_value,
        b_value,
        segments,
        scene.start_elapsed_ns,
        scene.stop_elapsed_ns
    );
    assert_projected_point_window(detail.point, x0, y0, dx, dy, path_cm, movement, label);
    detail.reply
}

fn assert_state_query(expected, label) {
    query_uart("?", expected, label)
}

fn assert_error_reply(cmd, label) {
    send_uart_expect(cmd, "Error", label)
}

fn prepare_basic_idle_scene() {
    boot_scene(3000, 56, 4.0);
}

fn configure_param_default_to_1_3_10() {
    next_page();
    next_page();
    adjust_up(3);
    select_param();
    adjust_up(2);
    next_page();
}

fn prepare_idle_param_1_3_10(freq_hz, distance_cm, voltage_v) {
    boot_scene(freq_hz, distance_cm, voltage_v);
    configure_param_default_to_1_3_10();
}

fn start_run_to_990_scene(freq_hz) {
    prepare_idle_param_1_3_10(freq_hz, 56, 4.0);
    set_destination(990, 0, "预置目的地 (990,0)");
    press_s4_expect_running("启动运行")
}

let start_108 = #{ press_ns: 0, elapsed_ns: 0 };
let start_148_0 = #{ press_ns: 0, elapsed_ns: 0 };
let start_148_148 = #{ press_ns: 0, elapsed_ns: 0 };
let wait_148_148 = #{ start_press_ns: 0, start_elapsed_ns: 0, stop_elapsed_ns: 0 };
let start_120_160 = #{ press_ns: 0, elapsed_ns: 0 };
let start_990_single = #{ press_ns: 0, elapsed_ns: 0 };
let start_990_multi = #{ press_ns: 0, elapsed_ns: 0 };
let led_148_0_2s = #{
    l1: false,
    l2: false,
    l3: false,
    l4: false,
    l5: false,
    l6: false,
    l7: false,
    l8: false,
};

ckpt(1, "频率值 3000 hz, 距离值 56 cm, 日间 (ADC: 4V)", "L 0- 0", || {
    prepare_basic_idle_scene();
    assert_coord_page_exact(0, 0, "初始坐标页")
});

ckpt(2, "空闲状态, 日间 (ADC: 4V)", "继电器断开", || {
    assert_bool_state(relay_on(), false, "空闲状态继电器");
    relay_summary()
});

ckpt(3, "空闲状态", "L1-L8 均熄灭", || {
    assert_led_pattern(false, false, false, false, false, false, false, false, "空闲状态指示灯");
    led_summary()
});

ckpt(4, "切换速度界面", "E2 -----", || {
    next_page();
    assert_speed_idle_page("空闲速度页")
});

ckpt(5, "切换运行界面", "P 1.0 0", || {
    next_page();
    assert_param_exact(1.0, 0, "默认参数页")
});

ckpt(6, "R 加 0.2", "P 1.2 0", || {
    adjust_up(2);
    assert_param_exact(1.2, 0, "R 参数加 0.2")
});

ckpt(7, "R 减 0.1", "P 1.1 0", || {
    adjust_down(1);
    assert_param_exact(1.1, 0, "R 参数减 0.1")
});

ckpt(8, "R 减 0.1, B 减 5", "P 1.0 -5", || {
    adjust_down(1);
    select_param();
    adjust_down(1);
    assert_param_exact(1.0, -5, "R 和 B 参数联调")
});

ckpt(9, "B 加 15", "P 1.0 10", || {
    adjust_up(3);
    assert_param_exact(1.0, 10, "B 参数加 15")
});

ckpt(10, "B 减 5", "P 1.0 5", || {
    adjust_down(1);
    assert_param_exact(1.0, 5, "B 参数加 5")
});

ckpt(11, "空闲状态", "L1-L8 均熄灭 (该状态下始终熄灭)", || {
    assert_led_pattern(false, false, false, false, false, false, false, false, "空闲状态指示灯保持");
    led_summary()
});

ckpt(12, "切换坐标界面, 串口发送: #", "串口返回: (0,0)", || {
    next_page();
    assert_position_query_exact(0, 0, "查询当前位置")
});

ckpt(13, "串口发送: ?", "串口返回: Idle", || {
    assert_state_query("Idle", "查询设备状态")
});

ckpt(14, "串口发送: (108,0)", "串口返回: Got it", || {
    boot_scene(500, 56, 4.0);
    set_destination(108, 0, "设置目的地")
});

ckpt(15, "坐标界面", "L 0- 0", || {
    assert_coord_page_exact(0, 0, "空闲坐标页仍显示当前位置")
});

ckpt(16, "切换运行状态, 1s 后, 串口返回: #", "串口返回: (16,0)", || {
    start_108 = press_s4_expect_running("启动运行");
    assert_projected_position_query_from_start(start_108, 0, 0, 108, 0, 108.0, 1.0, 0, 500, 1000, "运行 1s 后的位置")
});

ckpt(17, "运行中, 串口返回: ?", "串口返回: Busy", || {
    assert_state_query("Busy", "运行中状态查询")
});

ckpt(18, "运行中", "继电器吸合 (空闲状态下断开)", || {
    assert_bool_state(relay_on(), true, "运行状态继电器");
    relay_summary()
});

ckpt(19, "运行中, 夜间", "指示灯 L1, L2 点亮, 其余指示灯熄灭 (空闲状态下均熄灭)", || {
    set_voltage(AIN1, 1.0);
    sensor_settle();
    assert_led_pattern(true, true, false, false, false, false, false, false, "夜间运行指示灯");
    led_summary()
});

ckpt(20, "切换速度界面, 运行中", "E1 15.7", || {
    next_page();
    assert_speed_formula_page(1.0, 0, 500, "运行速度页")
});

ckpt(21, "运行中, 日间 (ADC: 4.5V)", "指示灯 L1 点亮, 其余指示灯熄灭 (夜间时, L2 点亮)", || {
    set_voltage_expect_led2(4.5, false, "夜间切日间");
    assert_led_pattern(true, false, false, false, false, false, false, false, "日间运行指示灯");
    led_summary()
});

ckpt(22, "7s 后, 自动切换空闲状态", "E2 ----- (同时运行时在速度值界面)", || {
    run_to_elapsed_from(start_108.press_ns, 8500);
    assert_speed_idle_page("到达后自动回到空闲速度页")
});

ckpt(23, "切换坐标界面", "继电器断开 (运行状态时吸合)", || {
    next_page();
    next_page();
    assert_bool_state(relay_on(), false, "到达后继电器断开");
    relay_summary()
});

ckpt(24, "空闲中", "L1-L8 均熄灭 (运行时 L1, L2 点亮, 其余指示灯熄灭)", || {
    assert_led_pattern(false, false, false, false, false, false, false, false, "到达后空闲指示灯");
    led_summary()
});

ckpt(25, "串口发送: #", "串口返回: (108,0)", || {
    assert_position_query_exact(108, 0, "到达后的当前位置")
});

ckpt(26, "频率值 800 hz, 串口返回: (148,0), 空闲状态", "L148- 0", || {
    set_frequency_hz(800);
    set_distance_cm(56);
    set_voltage(AIN1, 4.0);
    sensor_settle();
    set_destination(148, 0, "预置目的地 (148,0)");
    start_148_0 = press_s4_expect_running("启动运行");
    run_to_elapsed_from(start_148_0.press_ns, 2000);
    led_148_0_2s = led_snapshot();
    run_to_elapsed_from(start_148_0.press_ns, 5000);
    assert_coord_page_exact(148, 0, "到达 (148,0) 后的坐标页")
});

ckpt(27, "运行 2s 后 (V: 25.1, 1.6s 行进结束)", "L3 点亮, 其余指示灯熄灭", || {
    assert_led_snapshot_pattern(led_148_0_2s, false, false, true, false, false, false, false, false, "到达提示灯");
    led_snapshot_summary(led_148_0_2s)
});

ckpt(28, "行进结束 3s 后", "L1-L8 均熄灭 (行进结束 3s 内 L3 点亮)", || {
    assert_led_pattern(false, false, false, false, false, false, false, false, "到达 3s 后指示灯");
    led_summary()
});

ckpt(29, "串口发送: (148,148)", "串口返回: Got it", || {
    set_destination(148, 148, "设置目的地 (148,148)")
});

ckpt(30, "空闲状态", "L148- 0", || {
    assert_coord_page_exact(148, 0, "空闲状态仍显示当前位置")
});

ckpt(31, "切换运行状态", "L148-148", || {
    set_frequency_hz(800);
    set_distance_cm(100);
    set_voltage(AIN1, 4.0);
    sensor_settle();
    start_148_148 = press_s4_expect_running("按键切换运行");
    page_settle();
    assert_coord_page_exact(148, 148, "运行中的坐标页显示目的地")
});

ckpt(32, "串口发送: #, 2s 后", "串口返回: (148,50)", || {
    assert_projected_position_query_from_start(start_148_148, 148, 0, 0, 148, 148.0, 1.0, 0, 800, 2000, "运行 2s 后的位置")
});

ckpt(33, "1s 后, 串口返回: #", "串口返回: (148,75)", || {
    assert_projected_position_query_from_start(start_148_148, 148, 0, 0, 148, 148.0, 1.0, 0, 800, 3000, "运行 3s 后的位置")
});

ckpt(34, "按键切换等待状态", "L148-148", || {
    run_to_elapsed_from(start_148_148.press_ns, 3900);
    let stop = press_s4_expect_relay_off_detail("运行切到等待");
    wait_148_148 = #{
        start_press_ns: start_148_148.press_ns,
        start_elapsed_ns: start_148_148.elapsed_ns,
        stop_elapsed_ns: stop.elapsed_ns,
    };
    assert_coord_page_exact(148, 148, "等待状态坐标页显示目的地")
});

ckpt(35, "等待状态", "指示灯 L1 闪烁间隔为 0.1s", || {
    let stats = watch_led_stats(L1, 1000);
    assert_float_window(stats.change_frequency_hz, 8.0, 12.0, "等待状态 L1 闪烁频率");
    "change_frequency_hz=" + stats.change_frequency_hz
});

ckpt(36, "串口发送: ?", "串口返回: Wait", || {
    assert_state_query("Wait", "等待状态查询")
});

ckpt(37, "等待状态", "继电器断开 (运行时吸合, 空闲时断开)", || {
    assert_bool_state(relay_on(), false, "等待状态继电器");
    relay_summary()
});

ckpt(38, "切换速度界面, 距离值 56, 等待中", "E3 56", || {
    next_page();
    set_distance_expect_wait_page(56, "等待状态距离页 56cm")
});

ckpt(39, "速度界面, 距离值 78, 等待中", "E3 78", || {
    set_distance_expect_wait_page(78, "等待状态距离页 78cm")
});

ckpt(40, "等待中, 串口返回: #", "串口返回: (148,100)", || {
    let detail = query_uart_point_detail("等待状态位置保持");
    set_distance_cm(45);
    press_s4_expect_running("避障检查前恢复运行");
    trigger_obstacle_expect_wait(15, "障碍物触发等待");
    let movement = speed_segments_distance_window(
        1.0,
        0,
        [speed_segment(800, 3900)],
        wait_148_148.start_elapsed_ns,
        wait_148_148.stop_elapsed_ns
    );
    assert_projected_point_window(
        detail.point,
        148,
        0,
        0,
        148,
        148.0,
        movement,
        "等待状态位置保持"
    );
    detail.reply
});

ckpt(41, "速度界面, 距离值 15, 按键切换启动", "E3 15", || {
    assert_distance_formula_page(15, "等待状态距离页 15cm");
    set_key(S4, true);
    run_ms(80);
    set_key(S4, false);
    page_settle();
    assert_distance_formula_page(15, "障碍物未清除时仍保持等待")
});

ckpt(42, "切换坐标界面, 等待中", "L148-148", || {
    next_page();
    next_page();
    assert_coord_page_exact(148, 148, "等待状态坐标页")
});

ckpt(43, "等待中, 串口返回: ?", "串口返回: Wait", || {
    assert_state_query("Wait", "等待状态查询")
});

ckpt(44, "切换速度界面, 距离值 20 cm", "E3 20", || {
    next_page();
    set_distance_expect_wait_page(20, "等待状态距离页 20cm")
});

ckpt(45, "速度界面, 距离值 45 cm", "E3 45", || {
    set_distance_expect_wait_page(45, "等待状态距离页 45cm")
});

ckpt(46, "按键启动 (距离值 45 cm), 切换运行状态", "E1 25.1", || {
    press_s4_expect_running("等待恢复运行");
    run_ms(1200);
    assert_speed_formula_page(1.0, 0, 800, "障碍物清除后恢复运行")
});

ckpt(47, "切换参数界面, R 加 0.3, B 加 10", "P 1.3 10", || {
    run_ms(6000);
    next_page();
    adjust_up(3);
    select_param();
    adjust_up(2);
    assert_param_exact(1.3, 10, "参数页调整到 R=1.3,B=10")
});

ckpt(48, "切换坐标界面", "L148-148", || {
    next_page();
    assert_coord_page_exact(148, 148, "参数生效后回到坐标页")
});

ckpt(49, "重置 (未重置时, 坐标值为 (148,148))", "L 0- 0 (未重置前, 显示坐标值正确)", || {
    reset_position();
    assert_coord_page_exact(0, 0, "重置当前位置")
});

ckpt(50, "频率值 1400, 串口返回: (120,160), 切换运行状态, 1s 后, 串口返回: # (R: 1.3, B: 10)", "串口返回: (40,54)", || {
    set_frequency_hz(1400);
    set_distance_cm(56);
    set_voltage(AIN1, 4.0);
    sensor_settle();
    set_destination(120, 160, "预置目的地 (120,160)");
    start_120_160 = press_s4_expect_running("启动运行");
    assert_projected_position_query_from_start(start_120_160, 0, 0, 120, 160, 200.0, 1.3, 10, 1400, 1000, "运行 1s 后的位置")
});

ckpt(51, "1s 后, 串口返回: # (R: 1.3, B: 10)", "串口返回: (80,107)", || {
    assert_projected_position_query_from_start(start_120_160, 0, 0, 120, 160, 200.0, 1.3, 10, 1400, 2000, "运行 2s 后的位置")
});

ckpt(52, "1s 后, 串口返回: #", "串口返回: (120,160)", || {
    run_to_elapsed_from(start_120_160.press_ns, 3200);
    assert_position_query_exact(120, 160, "到达目的地后的当前位置")
});

ckpt(53, "空闲状态", "L120-160", || {
    assert_coord_page_exact(120, 160, "到达 (120,160) 后的坐标页")
});

ckpt(54, "重置, 频率值 600 hz, 串口返回: (990,0) (未重置时, 坐标值为 (120,160))", "L 0- 0 (未重置前, 显示坐标值正确)", || {
    reset_position();
    set_frequency_hz(600);
    sensor_settle();
    set_destination(990, 0, "重置后设置目的地 (990,0)");
    assert_coord_page_exact(0, 0, "重置后当前位置")
});

ckpt(55, "切换运行状态, 1s 后, 串口返回: # (R: 1.3, B: 10)", "串口返回: (35,0)", || {
    start_990_single = press_s4_expect_running("启动运行");
    assert_projected_position_query_from_start(start_990_single, 0, 0, 990, 0, 990.0, 1.3, 10, 600, 1000, "600Hz 运行 1s 后的位置")
});

ckpt(56, "切换速度界面", "E1 34.5", || {
    next_page();
    assert_speed_formula_page(1.3, 10, 600, "600Hz 运行速度页")
});

ckpt(57, "频率值 4000", "E1 173.3", || {
    set_frequency_hz(4000);
    freq_settle();
    assert_speed_formula_page(1.3, 10, 4000, "4000Hz 运行速度页")
});

ckpt(58, "串口发送: # (V: 34.5 运行 3s, V: 173.3 运行 1s)", "串口返回: (243,0)", || {
    let start = start_run_to_990_scene(600);
    run_to_elapsed_from(start.press_ns, 2000);
    set_frequency_hz(4000);
    assert_projected_position_query_from_segments(
        start,
        0,
        0,
        990,
        0,
        990.0,
        1.3,
        10,
        [speed_segment(600, 2000), speed_segment(4000, 1000)],
        "变速后的当前位置"
    )
});

ckpt(59, "频率值 1200", "E1 59.0", || {
    set_frequency_hz(1200);
    freq_settle();
    next_page();
    assert_speed_formula_page(1.3, 10, 1200, "1200Hz 运行速度页")
});

ckpt(60, "串口发送: # (V: 34.5 运行 3s, V: 173.3 运行 2.5s, V: 59.0 运行 1s)", "串口返回: (600,0)", || {
    start_990_multi = start_run_to_990_scene(600);
    run_to_elapsed_from(start_990_multi.press_ns, 3000);
    set_frequency_hz(4000);
    run_ms(2500);
    set_frequency_hz(1200);
    assert_projected_position_query_from_segments(
        start_990_multi,
        0,
        0,
        990,
        0,
        990.0,
        1.3,
        10,
        [speed_segment(600, 3000), speed_segment(4000, 2500), speed_segment(1200, 1000)],
        "多段变速后的当前位置"
    )
});

ckpt(61, "串口发送: (0,90), 运行中", "串口返回: Busy", || {
    send_uart_expect("(0,90)", "Busy", "运行中设置新目的地")
});

ckpt(62, "串口发送: (0,90) (V: 34.5 运行 3s, V: 173.3 运行 2.5s, V: 59.0 运行 4s)", "串口返回: (774,0)", || {
    assert_projected_position_query_from_segments(
        start_990_multi,
        0,
        0,
        990,
        0,
        990.0,
        1.3,
        10,
        [speed_segment(600, 3000), speed_segment(4000, 2500), speed_segment(1200, 4000)],
        "长时间变速后的当前位置"
    )
});

ckpt(63, "串口发送: 90", "串口返回: Error", || {
    boot_scene(600, 56, 4.0);
    assert_error_reply("90", "非法串口指令 90")
});

ckpt(64, "串口发送: w", "串口返回: Error", || {
    boot_scene(600, 56, 4.0);
    assert_error_reply("w", "非法串口指令 w")
});
