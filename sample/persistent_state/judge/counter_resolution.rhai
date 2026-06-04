key_mode(KEYBOARD);

fn tap_many(key, count) {
    let i = 0;
    while i < count {
        tap_key(key, 20);
        i += 1;
    }
    run_ms(120);
}

fn screen() {
    display_text(30)
}

fn assert_screen(vv_expected, r_expected, eee_expected, label) {
    let text = screen();
    assert(regex_is_match(text, "^\\d{2}-\\d-\\d{3}$"), label + " 显示格式");
    assert_eq(parse_int(text[0..2]), vv_expected, label + " VV");
    assert_eq(parse_int(text[3..4]), r_expected, label + " R");
    assert_eq(parse_int(text[5..8]), eee_expected, label + " EEE");
}

fn normalized_state() {
    set_rtc(1, 2, 3);
    export_persistent_state()
}

run_ms(240);
let boot_state = normalized_state();
assert_screen(0, 3, 0, "上电");

tap_many(S4, 99);
assert_screen(99, 3, 0, "S4 连按 99 次");
assert_eq(normalized_state(), boot_state, "VV 变化不应进入持久状态");

tap_many(S4, 1);
assert_screen(0, 3, 0, "S4 第 100 次回绕");
assert_eq(normalized_state(), boot_state, "VV 回绕后仍不应进入持久状态");

tap_many(S4, 1);
assert_screen(1, 3, 0, "S4 第 101 次");
assert_eq(normalized_state(), boot_state, "VV 增加 1 后仍不应进入持久状态");

reset(POWER_RESET);
run_ms(180);
assert_screen(0, 3, 0, "reset 后 VV 清零");
assert_eq(normalized_state(), boot_state, "reset 后持久状态仍应保持 boot");

tap_many(S6, 1);
assert_screen(0, 0, 0, "S6 从 3 回绕到 0");
let res0_state = normalized_state();
assert(res0_state != boot_state, "R 改变后持久状态字符串应变化");

reset(POWER_RESET);
run_ms(180);
assert_screen(0, 0, 0, "reset 后保留 R");
assert_eq(normalized_state(), res0_state, "reset 不应破坏 R 的持久状态");

tap_many(S5, 1);
assert_screen(0, 3, 0, "S5 从 0 回绕到 3");
let res3_written_state = normalized_state();
assert(res3_written_state != res0_state, "R 从 0 回到 3 后持久状态应再次变化");

tap_many(S5, 4);
assert_screen(0, 3, 0, "S5 循环 4 次");
assert_eq(normalized_state(), res3_written_state, "S5 循环一周后持久状态不变");

tap_many(S6, 4);
assert_screen(0, 3, 0, "S6 循环 4 次");
assert_eq(normalized_state(), res3_written_state, "S6 循环一周后持久状态不变");
