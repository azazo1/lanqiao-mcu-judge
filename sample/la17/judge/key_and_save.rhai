key_mode(KEYBOARD);

fn double_click(key) {
    tap_key(key, 80);
    run_ms(120);
    tap_key(key, 80);
    run_ms(220);
}

fn hold_save() {
    set_key(S12, true);
    set_key(S13, true);
    run_ms(2200);
    set_key(S12, false);
    set_key(S13, false);
    run_ms(180);
}

fn goto_temp_page() {
    tap_key(S4, 80);
    run_ms(120);
}

fn goto_light_page() {
    tap_key(S4, 80);
    run_ms(120);
    tap_key(S4, 80);
    run_ms(120);
}

set_rtc(#{
    year: 26,
    month: 6,
    date: 5,
    weekday: 4,
    hour: 23,
    minute: 8,
    second: 9,
    running: false,
});
set_eeprom([0x16, 0x71, 0x00, 20, 96, 0x49]);
reset(POWER_RESET);
run_ms(320);

assert_eq(display_text(30), "23-08-09", "默认 24h 时间显示");

double_click(S12);
assert_eq(display_text(30), "11-08-09", "S12 双击后切到 12h");

set_rtc(0, 8, 9);
reset(POWER_RESET);
run_ms(320);
assert_eq(display_text(30), "12-08-09", "00 点切到 12h 后显示 12");

double_click(S12);
assert_eq(display_text(30), "00-08-09", "双击后切回 24h");

set_rtc(12, 8, 9);
reset(POWER_RESET);
run_ms(320);
assert_eq(display_text(30), "12-08-09", "24h 下 12 点显示");

double_click(S12);
assert_eq(display_text(30), "12-08-09", "12 点切到 12h 后仍显示 12");

set_rtc(23, 8, 9);
reset(POWER_RESET);
run_ms(320);
assert_eq(display_text(30), "11-08-09", "恢复 23 点后仍保持 12h");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "26-06-05", "S5 短按后显示日期");

tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "11-08-09", "再次短按后回到 12h 时间页");

goto_temp_page();
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.500-0", "默认精度 0");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "25.750-1", "S8 后精度 1");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "25.875-2", "再次 S8 后精度 2");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "25.937-3", "再次 S8 后精度 3");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "25.500-0", "S8 后精度从 3 回绕到 0");

tap_key(S9, 80);
run_ms(200);
assert_eq(display_text(30), "25.937-3", "S9 后精度从 0 回绕到 3");

goto_light_page();
tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "0-032", "默认自动光控 DAC 子页面");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "0-032", "自动光控下 S8 不应改写 DAC");
assert_eq(da_value(), 32, "自动光控下 DAC 保持自动结果");

double_click(S12);
assert_eq(display_text(30), "1-096", "双击后切到手动光控");
assert_eq(da_value(), 96, "手动光控初始 DAC");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "1-104", "手动 DAC 加 8");
assert_eq(da_value(), 104, "DAC 应同步更新为 104");

let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 36..=45, "手动 DAC 为 104 时的 PWM 占空比");

hold_save();
assert_eq(eeprom_byte(0x02), 0x0F, "保存后的配置字节");
assert_eq(eeprom_byte(0x03), 20, "保存后音速偏移保持默认");
assert_eq(eeprom_byte(0x04), 104, "保存后的手动 DAC");
assert_eq(eeprom_byte(0x05), 0x4E, "保存后的校验字节");

reset(POWER_RESET);
run_ms(320);
assert_eq(display_text(30), "11-08-09", "重上电后仍保持 12h");

goto_temp_page();
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.937-3", "重上电后恢复精度 3");

goto_light_page();
tap_key(S5, 80);
run_ms(120);
assert_eq(display_text(30), "1-104", "重上电后恢复手动光控和 DAC");
assert_eq(da_value(), 104, "重上电后恢复 DAC");

let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 36..=45, "重上电后 PWM 占空比恢复");
