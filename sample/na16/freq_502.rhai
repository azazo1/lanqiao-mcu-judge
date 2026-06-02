run_ms(50);
set_frequency_hz(2200);
run_ms(1500);
assert(led_on(6), "频率异常时 L6 应点亮");
