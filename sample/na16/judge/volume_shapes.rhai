key_mode(KEYBOARD);

fn settle() {
    run_ms(320);
}

fn sensor_settle() {
    run_ms(1600);
}

fn goto_volume_page() {
    tap_key(S5, 80);
    run_ms(160);
    tap_key(S5, 80);
    run_ms(160);
}

fn assert_volume_page(spare_min, spare_max, volume_min, volume_max, label) {
    let text = display_text(30);
    assert_regex(text, "^\\s?\\d{1,3}-\\s{0,2}\\d{1,2}\\.\\d$", label + " 体积格式");
    assert_in(display_number(1, 3), spare_min..=spare_max, label + " 剩余空间");
    let volume = display_number(5, 8);
    assert(volume >= volume_min && volume <= volume_max, label + " 液体体积");
}

set_temperature_c(25);
set_distance_cm(35);
set_voltage(RB2, 2.3);
sensor_settle();
goto_volume_page();
assert_volume_page(34, 37, 0.5, 0.7, "默认长方体体积");

reset(POWER_RESET);
settle();
uart_take();
uart_write("(L,1.5)(W,1.0)(H,2.0)");
run_ms(160);
let reply_rect = uart_take();
assert(reply_rect[0..2] == "OK", "长方体参数配置");
set_temperature_c(25);
set_distance_cm(76);
sensor_settle();
goto_volume_page();
assert_volume_page(37, 40, 1.7, 1.9, "长方体体积 76cm");

reset(POWER_RESET);
settle();
uart_take();
uart_write("(S,1)");
run_ms(160);
assert_eq(uart_take(), "OK", "切换到圆柱体");
uart_write("(L,1.5)(H,2.0)");
run_ms(160);
let reply_cyl = uart_take();
assert(reply_cyl[0..2] == "OK", "圆柱体参数配置");
set_temperature_c(25);
set_distance_cm(76);
sensor_settle();
goto_volume_page();
assert_volume_page(38, 40, 3.8, 4.1, "圆柱体体积 76cm");

reset(POWER_RESET);
settle();
uart_take();
uart_write("(S,1)(H1,2.3)(H2,0.5)(L,3.0)(W,1.5)(H,2.5)(r,2.0)");
run_ms(220);
let reply_big = uart_take();
assert(reply_big[0..2] == "OK", "圆柱体大容器参数配置");
set_temperature_c(25);
set_distance_cm(87);
sensor_settle();
goto_volume_page();
assert_volume_page(34, 36, 20.2, 20.6, "圆柱体体积 87cm");

set_distance_cm(45);
sensor_settle();
assert_volume_page(17, 19, 25.5, 25.9, "圆柱体体积 45cm");
