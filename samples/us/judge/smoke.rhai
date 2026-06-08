fn settle() {
    run_ms(220);
}

fn tap_many(key, count) {
    let i = 0;
    while i < count {
        tap_key(key, 80);
        i += 1;
    }
}

fn assert_page(prefix, label) {
    assert_eq(display_text(30)[0..1], prefix, label + ": 页面前缀错误");
}

fn distance_value() {
    display_number(4, 8, 30)
}

fn speed_value() {
    display_number(6, 8, 30)
}

fn assert_speed(expected, label) {
    assert_page("P", label);
    assert_eq(speed_value(), expected, label + ": 音速数值错误");
}

fn assert_distance_range(lower, upper, label) {
    assert_page("L", label);
    assert_in(distance_value(), lower..=upper, label + ": 距离数值超出范围");
}

fn assert_distance_formula(distance_cm, speed, tolerance, label) {
    let expected = distance_cm * speed / 340;
    assert_distance_range(expected - tolerance, expected + tolerance, label);
}

settle();
assert_page("L", "上电默认距离页");
assert_eq(distance_value(), 0, "上电检查: 默认距离值错误");

set_distance_cm(20);
settle();
assert_distance_range(18, 20, "20cm 在默认音速下的距离显示");

tap_key(S4, 80);
settle();
assert_speed(340, "切换到音速页后默认音速");

set_distance_cm(123);
settle();
assert_speed(340, "音速页不应被距离变化干扰");

tap_key(S9, 80);
settle();
assert_speed(345, "S9 单步加 5");

tap_key(S8, 80);
settle();
assert_speed(340, "S8 单步减 5");

tap_many(S9, 12);
settle();
assert_speed(400, "音速增加到上限 400");

tap_many(S9, 2);
settle();
assert_speed(400, "超过上限后应保持 400");

tap_key(S4, 80);
settle();
assert_distance_formula(123, 400, 1, "400m/s 时 123cm 的距离显示");

set_distance_cm(100);
settle();
assert_distance_formula(100, 400, 1, "400m/s 时 100cm 的距离显示");

reset();
settle();
assert_page("L", "复位后回到距离页");
assert_distance_formula(100, 340, 1, "复位后保留当前距离注入");

tap_key(S4, 80);
settle();
assert_speed(340, "复位后音速恢复默认值");

tap_many(S8, 8);
settle();
assert_speed(300, "音速减小到下限 300");

tap_many(S8, 2);
settle();
assert_speed(300, "低于下限后应保持 300");

set_distance_cm(200);
settle();
assert_speed(300, "最低音速页不应被距离变化干扰");

tap_key(S4, 80);
settle();
set_distance_cm(100);
settle();
assert_distance_formula(100, 300, 1, "300m/s 时 100cm 的距离显示");

set_distance_cm(0);
settle();
assert_eq(distance_value(), 0, "0cm 输入: 距离显示错误");
