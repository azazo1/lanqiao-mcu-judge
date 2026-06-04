run_ms(100);
assert_eq(eeprom_byte(0), 0xA5, "写入 eeprom 地址 0");
for i in 1..8 {
    assert_eq(led_on(i), false, "L" + (i) + " 应该熄灭");
}