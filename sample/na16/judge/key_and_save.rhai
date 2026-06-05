key_mode(KEYBOARD);

fn double_click(key) {
    tap_key(key, 80);
    run_ms(120);
    tap_key(key, 80);
    run_ms(220);
}

fn hold_save() {
    set_key(S8, true);
    set_key(S9, true);
    run_ms(2200);
    set_key(S8, false);
    set_key(S9, false);
    run_ms(180);
}

fn goto_temp_page() {
    tap_key(S4, 80);
    run_ms(120);
}

fn goto_ad_page() {
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
set_eeprom([0xA5, 0x5A, 0x00, 40, 128, 0xA7]);
reset(POWER_RESET);
run_ms(320);

assert_eq(display_text(30), "23-08-09", "默认 24h 时间显示");

double_click(S6);
assert_eq(display_text(30), "11-08-09", "S6 双击后切到 12h");

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

tap_key(S7, 80);
run_ms(200);
assert_eq(display_text(30), "25.750-1", "S7 后精度 1");

tap_key(S7, 80);
run_ms(200);
assert_eq(display_text(30), "25.875-2", "再次 S7 后精度 2");

tap_key(S8, 80);
run_ms(200);
assert_eq(display_text(30), "25.750-1", "S8 后精度回到 1");

reset(POWER_RESET);
run_ms(320);
goto_ad_page();
let ad_text = display_text(30);
assert_eq(da_value(), 128, "默认 DAC 值");

let duty0 = watch_led_stats(L6, 40);
assert_in(duty0.duty_percent, 45..=55, "默认 DAC 对应的 L6 占空比");

tap_key(S7, 80);
run_ms(220);
assert_eq(da_value(), 144, "AD 页 S7 后 DAC 增加 16");
assert_eq(display_text(30), ad_text, "DAC 调节不应改写 AD 页面");

let duty1 = watch_led_stats(L6, 40);
assert_in(duty1.duty_percent, 52..=60, "DAC 增大后的 L6 占空比");

hold_save();
assert_eq(eeprom_byte(0x02), 0x03, "保存后的配置字节");
assert_eq(eeprom_byte(0x03), 40, "保存后音速配置保持默认");
assert_eq(eeprom_byte(0x04), 144, "保存后的 DAC");
assert_eq(eeprom_byte(0x05), 0xB7, "保存后的校验和");

reset(POWER_RESET);
run_ms(320);
assert_eq(display_text(30), "11-08-09", "重上电后仍保持 12h");

goto_temp_page();
set_temperature_c(25.9375);
run_ms(260);
assert_eq(display_text(30), "25.750-1", "重上电后恢复精度 1");

reset(POWER_RESET);
run_ms(320);
goto_ad_page();
assert_eq(da_value(), 144, "重上电后恢复 DAC");
let duty2 = watch_led_stats(L6, 40);
assert_in(duty2.duty_percent, 52..=60, "重上电后 L6 占空比恢复");
