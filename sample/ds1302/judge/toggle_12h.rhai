run_ms(200);
tap_key(S12, 30);
run_ms(200);
assert_eq(display_number(3, 4), 1, "13 点切换之后应该是下午 1 点");
