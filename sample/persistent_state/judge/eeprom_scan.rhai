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

fn expected_value(addr) {
    if addr == 0 {
        1
    } else if addr == 1 {
        2
    } else if addr == 2 {
        255
    } else if addr == 15 {
        255
    } else if addr == 16 {
        1
    } else if addr == 31 {
        7
    } else if addr == 32 {
        255
    } else {
        0
    }
}

run_ms(240);
let boot_state = export_persistent_state();
assert_screen(0, 3, 0, "上电");

tap_many(S7, 1);
assert_screen(0, 3, 0, "S7 从 0x00 回绕到 0x20");

tap_many(S8, 1);
assert_screen(0, 3, 0, "S8 从 0x20 回到 0x00");

tap_many(S10, 1);
assert_screen(0, 3, 1, "地址 0x00 写入 1");
assert_eq(eeprom_byte(0), 1, "EEPROM[0x00]");

load_persistent_state(boot_state);
reset(POWER_RESET);
run_ms(180);
assert_screen(0, 3, 0, "恢复 boot 后");

tap_many(S10, 1);
assert_screen(0, 3, 1, "地址 0x00 设为 1");

tap_many(S8, 1);
tap_many(S10, 2);
assert_screen(0, 3, 2, "地址 0x01 设为 2");

tap_many(S8, 1);
tap_many(S9, 1);
assert_screen(0, 3, 255, "地址 0x02 下溢到 255");

tap_many(S8, 13);
tap_many(S9, 1);
assert_screen(0, 3, 255, "地址 0x0F 下溢到 255");

tap_many(S8, 1);
tap_many(S10, 1);
assert_screen(0, 3, 1, "地址 0x10 设为 1");

tap_many(S8, 15);
tap_many(S9, 1);
tap_many(S10, 8);
assert_screen(0, 3, 7, "地址 0x1F 经过上溢后为 7");

tap_many(S8, 1);
tap_many(S9, 1);
assert_screen(0, 3, 255, "地址 0x20 下溢到 255");

assert_eq(eeprom_byte(0), 1, "地址 0x00");
assert_eq(eeprom_byte(1), 2, "地址 0x01");
assert_eq(eeprom_byte(2), 255, "地址 0x02");
assert_eq(eeprom_byte(15), 255, "地址 0x0F 应命中 255 边界");
assert_eq(eeprom_byte(16), 1, "地址 0x10 应保持 1");
assert_eq(eeprom_byte(31), 7, "地址 0x1F 应经过上溢后得到 7");
assert_eq(eeprom_byte(32), 255, "地址 0x20 应保留块上界值");
assert_eq(eeprom_byte(33), 0, "地址 0x21 不应被写到");
assert_eq(eeprom_byte(127), 0, "地址 0x7F 不应被写到");
assert_eq(eeprom_byte(255), 0, "地址 0xFF 不应被写到");

let addr = 0;
while addr <= 32 {
    assert_eq(eeprom_byte(addr), expected_value(addr), "EEPROM 块内容");
    addr += 1;
}

tap_many(S8, 1);
assert_screen(0, 3, 1, "S8 从 0x20 回绕到 0x00");

let scan_addr = 0;
while scan_addr < 32 {
    assert_screen(0, 3, expected_value(scan_addr), "S8 顺序扫描块内容");
    tap_many(S8, 1);
    scan_addr += 1;
}
assert_screen(0, 3, expected_value(32), "S8 顺序扫描块内容");

tap_many(S8, 1);
assert_screen(0, 3, 1, "S8 从 0x20 回绕到 0x00");

tap_many(S7, 1);
assert_screen(0, 3, 255, "S7 从 0x00 回绕到 0x20");
