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
add_marker("boot");
let boot_state = normalized_state();
add_marker("boot_state_exported");
assert_screen(0, 3, 0, "上电");

tap_many(S10, 1);
tap_many(S8, 1);
tap_many(S10, 2);
tap_many(S8, 1);
tap_many(S10, 3);
tap_many(S6, 1);
tap_many(S4, 12);
assert_screen(12, 0, 3, "构造 snapshot A");
let state_a = normalized_state();
add_marker("snapshot_a_exported");
assert(state_a != boot_state, "snapshot A 应不同于 boot");

reset(POWER_RESET);
run_ms(180);
add_marker("snapshot_a_restored_after_reset");
assert_screen(0, 0, 1, "reset 后恢复 snapshot A");
assert_eq(normalized_state(), state_a, "reset 后 persistent state 不变");

tap_many(S10, 3);
tap_many(S8, 1);
tap_many(S9, 2);
tap_many(S8, 1);
tap_many(S9, 4);
tap_many(S5, 1);
tap_many(S4, 7);
let visible_b = screen();
assert_screen(7, 3, 255, "构造 snapshot B");
let state_b = normalized_state();
add_marker("snapshot_b_exported");
assert(state_b != state_a, "snapshot B 应不同于 snapshot A");

load_persistent_state(state_a);
assert_eq(normalized_state(), state_a, "load 后持久状态应立即恢复为 snapshot A");
assert_eq(eeprom_byte(0), 1, "load A 后 EEPROM[0x00]");
assert_eq(eeprom_byte(1), 2, "load A 后 EEPROM[0x01]");
assert_eq(eeprom_byte(2), 3, "load A 后 EEPROM[0x02]");
assert_eq(screen(), visible_b, "仅 load 不应自动刷新 RAM 缓存显示");

reset(POWER_RESET);
run_ms(180);
add_marker("state_a_visible_after_reset");
assert_screen(0, 0, 1, "load A 再 reset 后");
tap_many(S8, 1);
assert_screen(0, 0, 2, "snapshot A 中地址 0x01");
tap_many(S8, 1);
assert_screen(0, 0, 3, "snapshot A 中地址 0x02");

load_persistent_state(state_b);
reset(POWER_RESET);
run_ms(180);
add_marker("state_b_visible_after_reset");
assert_screen(0, 3, 4, "load B 再 reset 后");
tap_many(S8, 1);
assert_screen(0, 3, 0, "snapshot B 中地址 0x01");
tap_many(S8, 1);
assert_screen(0, 3, 255, "snapshot B 中地址 0x02");

load_persistent_state(boot_state);
reset(POWER_RESET);
run_ms(180);
add_marker("boot_state_visible_after_reset");
assert_screen(0, 3, 0, "恢复 boot 后");
assert_eq(eeprom_byte(0), 0, "恢复 boot 后 EEPROM[0x00]");
assert_eq(eeprom_byte(1), 0, "恢复 boot 后 EEPROM[0x01]");
assert_eq(eeprom_byte(2), 0, "恢复 boot 后 EEPROM[0x02]");
