key_mode(KEYBOARD);

fn tap_once(key) {
    tap_key(key, 80);
    run_ms(120);
}

fn screen() {
    display_text(30)
}

fn assert_screen(vv_expected, r_expected, eee_expected, label) {
    let text = screen();
    assert(regex_is_match(text, "^\\d{2}-\\d-\\d{3}$"), label + " 显示格式");
    assert_eq(text[2..3], "-", label + " 第3位固定为 -");
    assert_eq(text[4..5], "-", label + " 第5位固定为 -");
    assert_eq(parse_int(text[0..2]), vv_expected, label + " VV");
    assert_eq(parse_int(text[3..4]), r_expected, label + " R");
    assert_eq(parse_int(text[5..8]), eee_expected, label + " EEE");
}

add_marker("boot");
run_ms(240);
add_marker("boot_display_ready");
assert_screen(0, 3, 0, "上电");
assert_eq(eeprom_byte(0), 0, "上电 EEPROM[0x00]");
assert_eq(eeprom_byte(1), 0, "上电 EEPROM[0x01]");

tap_once(S4);
assert_screen(1, 3, 0, "S4 单击");

tap_once(S5);
assert_screen(1, 2, 0, "S5 单击");

tap_once(S6);
assert_screen(1, 3, 0, "S6 单击");

tap_once(S10);
add_marker("write_addr_00_inc");
assert_screen(1, 3, 1, "S10 单击");
assert_eq(eeprom_byte(0), 1, "S10 后改写当前地址");
assert_eq(eeprom_byte(1), 0, "S10 不应改写下一个地址");

tap_once(S7);
add_marker("cursor_wrap_to_20");
assert_screen(1, 3, 0, "S7 回绕到地址 0x20");

tap_once(S8);
assert_screen(1, 3, 1, "S8 回到地址 0x00");

tap_once(S9);
assert_screen(1, 3, 0, "S9 从 1 减到 0");
assert_eq(eeprom_byte(0), 0, "S9 后改写当前地址");

tap_once(S9);
add_marker("write_addr_00_underflow");
assert_screen(1, 3, 255, "S9 从 0 回绕到 255");
assert_eq(eeprom_byte(0), 255, "S9 需要支持 8bit 下溢回绕");
