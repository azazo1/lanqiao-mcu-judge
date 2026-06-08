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

add_marker("boot");
run_ms(240);
let boot_state = export_persistent_state();
add_marker("boot_state_exported");
assert_screen(0, 3, 0, "上电");

tap_many(S7, 1);
assert_screen(0, 3, 0, "S7 从 0x00 回绕到 0x20");

tap_many(S8, 1);
assert_screen(0, 3, 0, "S8 从 0x20 回到 0x00");

tap_many(S10, 1);
add_marker("write_00_to_01");
assert_screen(0, 3, 1, "地址 0x00 写入 1");
assert_eq(eeprom_byte(0), 1, "地址 0x00 写入后: EEPROM 数据错误");

load_persistent_state(boot_state);
reset(POWER_RESET);
run_ms(180);
add_marker("boot_state_restored");
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
add_marker("eeprom_block_written");
assert_screen(0, 3, 255, "地址 0x20 下溢到 255");

assert_eq(eeprom_byte(0), 1, "地址 0x00: EEPROM 数据错误");
assert_eq(eeprom_byte(1), 2, "地址 0x01: EEPROM 数据错误");
assert_eq(eeprom_byte(2), 255, "地址 0x02: EEPROM 数据错误");
assert_eq(eeprom_byte(15), 255, "地址 0x0F: 255 边界值错误");
assert_eq(eeprom_byte(16), 1, "地址 0x10 应保持 1");
assert_eq(eeprom_byte(31), 7, "地址 0x1F: 上溢结果错误");
assert_eq(eeprom_byte(32), 255, "地址 0x20: 块上界值保持错误");
assert_eq(eeprom_byte(33), 0, "地址 0x21: EEPROM 被错误写入");
assert_eq(eeprom_byte(127), 0, "地址 0x7F: EEPROM 被错误写入");
assert_eq(eeprom_byte(255), 0, "地址 0xFF: EEPROM 被错误写入");

let addr = 0;
while addr <= 32 {
    assert_eq(eeprom_byte(addr), expected_value(addr), "EEPROM 块扫描: 地址 " + addr + " 数据错误");
    addr += 1;
}

tap_many(S8, 1);
assert_screen(0, 3, 1, "S8 从 0x20 回绕到 0x00");

let scan_addr = 0;
add_marker("sequential_scan_begin");
while scan_addr < 32 {
    assert_screen(0, 3, expected_value(scan_addr), "S8 顺序扫描块内容");
    tap_many(S8, 1);
    scan_addr += 1;
}
add_marker("sequential_scan_end");
assert_screen(0, 3, expected_value(32), "S8 顺序扫描块内容");

tap_many(S8, 1);
assert_screen(0, 3, 1, "S8 从 0x20 回绕到 0x00");

tap_many(S7, 1);
assert_screen(0, 3, 255, "S7 从 0x00 回绕到 0x20");
