key_mode(BUTTON);

set_voltage(RB2, 0.0);
set_voltage(RB3, 0.0);
set_voltage(RB4, 0.0);
set_voltage(RD1, 0.0);
run_ms(500);
assert(regex_is_match(display_text(30), "^\\d{3} \\d{3}$"), "AD 显示格式");
let boot_text = display_text();
assert(parse_int(boot_text[0..3]) >= 0 && parse_int(boot_text[0..3]) <= 1, "左侧 AD 零点显示");
assert(parse_int(boot_text[4..7]) >= 0 && parse_int(boot_text[4..7]) <= 1, "右侧 AD 零点显示");
assert(da_value() == 127, "上电后 DA 原始值");

set_voltage(RB2, 1.0);
set_voltage(RB3, 2.0);
set_voltage(RB4, 3.0);
set_voltage(RD1, 4.0);
run_ms(500);
let first_text = display_text(30);
assert(parse_int(first_text[0..3]) >= 102 && parse_int(first_text[0..3]) <= 104, "左侧 AD 第一组输入");
assert(parse_int(first_text[4..7]) >= 152 && parse_int(first_text[4..7]) <= 154, "右侧 AD 第一组输入");

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

set_voltage(RB2, 4.0);
set_voltage(RB3, 3.0);
set_voltage(RB4, 2.0);
set_voltage(RD1, 1.0);
run_ms(500);
let second_text = display_text(30);
assert(parse_int(second_text[0..3]) >= 152 && parse_int(second_text[0..3]) <= 154, "左侧 AD 第二组输入");
assert(parse_int(second_text[4..7]) >= 102 && parse_int(second_text[4..7]) <= 104, "右侧 AD 第二组输入");

let j = 0;
while j < 24 {
    tap_key(S6, 80);
    j += 1;
}
run_ms(300);
assert(da_value() == base_da - 8, "S6 连按后 DA 应明显减小");
