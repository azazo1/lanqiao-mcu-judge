key_mode(BUTTON);

set_voltage(AIN3, 0.0);
set_voltage(AIN1, 0.0);
run_ms(500);
add_marker("adc_zero_sampled");
assert_regex(display_text(30), "^\\d{3} \\d{3}$", "上电采样: AD 显示格式错误");
let boot_text = display_text();
assert_in(parse_int(boot_text[0..3]), 0..=1, "上电采样: 左侧 AD 零点显示超出范围");
assert_in(parse_int(boot_text[4..7]), 0..=1, "上电采样: 右侧 AD 零点显示超出范围");
assert_eq(da_value(), 127, "上电采样: DA 原始值错误");

set_voltage(AIN3, 1.0);
set_voltage(AIN1, 4.0);
run_ms(500);
add_marker("adc_display_1v_4v");
let first_text = display_text(30);
assert_in(parse_int(first_text[0..3]), 50..=52, "第一组输入: 左侧 AD 显示超出范围");
assert_in(parse_int(first_text[4..7]), 203..=205, "第一组输入: 右侧 AD 显示超出范围");

let base_da = da_value();
assert_in(base_da, 123..=130, "第一组输入: 初始 DA 数值超出范围");
let i = 0;
while i < 16 {
    tap_key(S7, 80);
    i += 1;
}
run_ms(300);
add_marker("da_increment_settled");
assert_eq(da_value(), base_da + 16, "S7 连按: DA 增量错误");
assert_eq(display_text(30), first_text, "S7 连按: AD 显示被错误改写");

set_voltage(AIN3, 4.0);
set_voltage(AIN1, 1.0);
run_ms(500);
add_marker("adc_display_4v_1v");
let second_text = display_text(30);
assert_in(parse_int(second_text[0..3]), 203..=205, "第二组输入: 左侧 AD 显示超出范围");
assert_in(parse_int(second_text[4..7]), 50..=52, "第二组输入: 右侧 AD 显示超出范围");

let j = 0;
while j < 24 {
    tap_key(S6, 80);
    j += 1;
}
run_ms(300);
add_marker("da_decrement_settled");
assert_eq(da_value(), base_da - 8, "S6 连按: DA 减量错误");
