key_mode(BUTTON);

set_voltage(AIN3, 0.0);
set_voltage(AIN1, 0.0);
run_ms(500);
assert(regex_is_match(display_text(30), "^\\d{3} \\d{3}$"), "AD 显示格式");
let boot_text = display_text();
assert(parse_int(boot_text[0..3]) >= 0 && parse_int(boot_text[0..3]) <= 1, "左侧 AD 零点显示");
assert(parse_int(boot_text[4..7]) >= 0 && parse_int(boot_text[4..7]) <= 1, "右侧 AD 零点显示");
assert(da_value() == 127, "上电后 DA 原始值");

set_voltage(AIN3, 1.0);
set_voltage(AIN1, 4.0);
run_ms(500);
let first_text = display_text(30);
assert(parse_int(first_text[0..3]) >= 50 && parse_int(first_text[0..3]) <= 52, "左侧 AD 第一组输入");
assert(parse_int(first_text[4..7]) >= 203 && parse_int(first_text[4..7]) <= 205, "右侧 AD 第一组输入");

let base_da = da_value();
assert(base_da >= 123 && base_da <= 130, "初始 DA 应该在 127 左右");
let i = 0;
while i < 16 {
    tap_key(S7, 80);
    i += 1;
}
run_ms(300);
assert(da_value() == base_da + 16, "S7 连按后 DA 应明显增大");
assert_eq_str(display_text(30), first_text, "调节 DA 不应改写 AD 显示");

set_voltage(AIN3, 4.0);
set_voltage(AIN1, 1.0);
run_ms(500);
let second_text = display_text(30);
assert(parse_int(second_text[0..3]) >= 203 && parse_int(second_text[0..3]) <= 205, "左侧 AD 第二组输入");
assert(parse_int(second_text[4..7]) >= 50 && parse_int(second_text[4..7]) <= 52, "右侧 AD 第二组输入");

let j = 0;
while j < 24 {
    tap_key(S6, 80);
    j += 1;
}
run_ms(300);
assert(da_value() == base_da - 8, "S6 连按后 DA 应明显减小");
