run_ms(10);
assert_eq(eeprom_byte(0), 0xA5, "写入 eeprom 地址 0");

fn to_int(b) {
    if b { 1 } else { 0 }
}

print("LED: "
    + to_int(led_on(1))
    + to_int(led_on(2))
    + to_int(led_on(3))
    + to_int(led_on(4))
    + to_int(led_on(5))
    + to_int(led_on(6))
    + to_int(led_on(7))
    + to_int(led_on(8))
);

for i in 1..=8 {
    assert_eq(led_on(i), ((0xA5 >> (8-i)) & 1) == 1, "L" + (i));
}