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
    assert_regex(text, "^\\d{2}-\\d-\\d{3}$", label + ": 显示格式错误");
    assert_eq(parse_int(text[0..2]), vv_expected, label + ": VV 显示错误");
    assert_eq(parse_int(text[3..4]), r_expected, label + ": R 显示错误");
    assert_eq(parse_int(text[5..8]), eee_expected, label + ": EEE 显示错误");
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
assert_eq(normalized_state(), boot_state, "VV 变化后: 持久状态被错误改写");

tap_many(S4, 1);
assert_screen(0, 3, 0, "S4 第 100 次回绕");
assert_eq(normalized_state(), boot_state, "VV 回绕后: 持久状态被错误改写");

tap_many(S4, 1);
assert_screen(1, 3, 0, "S4 第 101 次");
assert_eq(normalized_state(), boot_state, "VV 增加 1 后: 持久状态被错误改写");

reset(POWER_RESET);
run_ms(180);
assert_screen(0, 3, 0, "reset 后 VV 清零");
assert_eq(normalized_state(), boot_state, "reset 后: 持久状态恢复错误");

tap_many(S6, 1);
assert_screen(0, 0, 0, "S6 从 3 回绕到 0");
let res0_state = normalized_state();
assert(res0_state != boot_state, "R 变化持久化: 持久状态未变化错误, boot=" + boot_state + ", actual=" + res0_state);

reset(POWER_RESET);
run_ms(180);
assert_screen(0, 0, 0, "reset 后保留 R");
assert_eq(normalized_state(), res0_state, "reset 后: R 的持久状态恢复错误");

tap_many(S5, 1);
assert_screen(0, 3, 0, "S5 从 0 回绕到 3");
let res3_written_state = normalized_state();
assert(res3_written_state != res0_state, "R 回绕持久化: 持久状态未变化错误, previous=" + res0_state + ", actual=" + res3_written_state);

tap_many(S5, 4);
assert_screen(0, 3, 0, "S5 循环 4 次");
assert_eq(normalized_state(), res3_written_state, "S5 循环一周后: 持久状态被错误改写");

tap_many(S6, 4);
assert_screen(0, 3, 0, "S6 循环 4 次");
assert_eq(normalized_state(), res3_written_state, "S6 循环一周后: 持久状态被错误改写");
