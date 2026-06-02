run_ms(220);
assert_eq_int(seg_pattern(1), 0x38, "上电默认显示 L");
assert(display_number(2, 8, 30) == 0, "上电默认距离");

set_distance_cm(20);
run_ms(220);
assert_eq_int(seg_pattern(1), 0x38, "距离页保持 L");
let default_dist = display_number(4, 8, 30);
assert(default_dist >= 18 && default_dist <= 20, "20cm 障碍物在默认音速下的距离显示");

tap_key(S4, 80);
run_ms(220);
assert_eq_int(seg_pattern(1), 0x73, "S4 切换到音速页");
assert(display_number(6, 8, 30) == 340, "默认音速值");

tap_key(S9, 80);
run_ms(220);
assert_eq_int(seg_pattern(1), 0x73, "音速页保持 P");
assert(display_number(6, 8, 30) == 345, "S9 增大音速");

tap_key(S8, 80);
run_ms(220);
assert(display_number(6, 8, 30) == 340, "S8 减小音速");

tap_key(S9, 80);
run_ms(220);
assert(display_number(6, 8, 30) == 345, "再次增大音速");

tap_key(S4, 80);
run_ms(220);
assert_eq_int(seg_pattern(1), 0x38, "切回距离页");
let adjusted_dist = display_number(4, 8, 30);
assert(adjusted_dist >= 19 && adjusted_dist <= 21, "音速变化后重新测得距离");
