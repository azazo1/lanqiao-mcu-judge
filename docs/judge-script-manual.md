# Rhai 评测脚本手册

## 目录约定

- 每个题目的评测脚本放在 `sample/xxx/judge/`.
- 推荐把冒烟脚本命名为 `smoke.rhai`.

## 运行方式

本文默认已经拿到编译后的可执行文件 `stcjudge`. 例如执行 `cargo build --release` 后, 二进制路径为 `target/release/stcjudge`.

脚本文件:

```bash
stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --script sample/key_seg/judge/smoke.rhai
```

标准输入:

```bash
stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --stdin < sample/key_seg/judge/smoke.rhai
```

交互式 REPL:

```bash
stcjudge repl --hex sample/key_seg/prj/Objects/key_seg.hex
```

脚本逐语句 tracing:

```bash
RUST_LOG=debug stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --script sample/key_seg/judge/smoke.rhai
```

REPL 内置命令:

- `:help`
- `:quit`
- `:exit`

打开 `RUST_LOG=debug` 后, 评测器会输出 Rhai 脚本的逐语句执行进度, 包括:

- 当前脚本标签
- 步号
- 行号和列号
- 调用层级
- 当前语句所在源码行

## 内置常量

LED:

- `L1` `L2` `L3` `L4` `L5` `L6` `L7` `L8`

按键:

- `S4` `S5` `S6` `S7`
- `S8` `S9` `S10` `S11`
- `S12` `S13` `S14` `S15`
- `S16` `S17` `S18` `S19`

模拟量通道:

- `RB2` `RD1` `AIN1` `AIN3`

其中 `AIN1` 和 `RD1` 是同一路输入的别名, `AIN3` 和 `RB2` 也是同一路输入的别名.

按键模式:

- `KEYBOARD` `KBD`
- `BUTTON` `BTN`

跳帽信号:

- `SIG_OUT`
- `NET_SIG`

这些常量可以直接传给脚本函数, 不需要再写成字符串.

## 执行控制

- `run_ms(ms)`
- `run_us(us)`

它们只推进仿真时间, 不等待真实时间.

`run_ms(...)` 和 `run_us(...)` 按精确仿真时基推进. 如果固件内部有显示刷新周期, 1s 测频窗口, 传感器采样节拍等逻辑, 在修改输入后要显式留出足够稳定时间, 不要假设几十毫秒内一定已经更新到最终结果.

## 输入注入

- `set_key(S4, true)`
- `set_key("S4", true)`
- `tap_key(S4, 50)`
- `key_mode(BTN)`
- `key_mode("kbd")`
- `set_rtc(23, 59, 50)`
- `set_temperature_c(25)`
- `set_distance_cm(35)`
- `set_frequency_hz(2200)`
- `set_voltage(AIN3, 2.3)`
- `set_voltage("AIN1", 2.3)`
- `uart_write("(F,?)")`
- `jumper_on(NET_SIG, SIG_OUT)`
- `jumper_off(NET_SIG, SIG_OUT)`
- `jumper_installed(NET_SIG, SIG_OUT)`

默认跳帽状态按开发板原理图处理. 例如 `NET_SIG` 和 `SIG_OUT` 在板内默认没有硬连, 所以仅仅 `set_frequency_hz(...)` 不会自动影响 `P3.4/T0`. 如果题目或评测需要把 NE555 输出接到单片机频率输入, 需要先显式调用 `jumper_on(NET_SIG, SIG_OUT)`.

## 输出观察

- `display_text()`
- `display_text(window_ms)`
- `display_number()`
- `display_number(window_ms)`
- `display_number(start, end)`
- `display_number(start, end, window_ms)`
- `snapshot_text()`
- `uart_take()`
- `da_value()`
- `relay_on()`
- `buzzer_on()`
- `motor_on()`
- `led_on(1)`
- `led_on(L1)`

`display_text()` 返回当前已经采样到的稳定显示结果. 当前实现不是去读某一个瞬时扫描相位, 而是维护每一位最近一次被锁存到的段码, 所以对于静态显示题目通常已经足够稳定.

`display_text(window_ms)` 会推进仿真时间, 在给定窗口内持续观察显示内容:

- 如果整个窗口内显示文本保持不变, 返回该文本.
- 如果窗口内文本发生变化, 直接报错.

这样更适合判断一段时间内显示是否稳定, 而不是只看某个瞬间.

`display_number()` 和 `display_number(window_ms)` 会从当前显示文本里提取唯一的整数. 如果显示内容里没有数字, 或者同时出现了多个数字, 会直接报错.

如果一屏里同时存在多个数字片段, 可以改用 `display_number(start, end)` 或 `display_number(start, end, window_ms)`, 在指定的数码管位范围内提取数字. 位号和 `seg_pattern(1)` 一样, 都是从左到右按 `1..=8` 计数.

`display_number(...)` 接受前导零, 但返回值只保留数值本身. 如果题目要求精确判断位宽, 前导零, 空白位, 固定符号位, 请直接对 `display_text(...)` 的结果做字符串切片和正则判断.

这更适合超声波, 温度, 电压这类量测题, 可以直接写布尔表达式判断范围, 避免按整串字符串做数值断言.

`da_value()` 返回当前 DA 输出的原始数值, 范围是 `0..=255`. 对 `PCF8591` 这类 AD/DA 题, 可以直接用它验证按键调节后的 DA 输出是否正确.

## Rhai 字符串切片和正则

- `regex_is_match(text, pattern)`
- `regex_match(text, pattern)`

Rhai 自带字符串切片语法, 可以直接写 `text[0..5]`. 这里的范围是 `start..end`, 也就是 0 基, 右边界不包含在结果里.

Rhai 也自带数值解析函数, 例如:

- `parse_int("123")`
- `parse_float("123.45")`
- `parse_int("ff", 16)`

推荐写法:

- 数值部分优先用 `display_text(...)[start..end]` 再配合 `parse_int(...)` 或 `parse_float(...)`.
- 固定字符, 空白位, 前导零, 分隔符等格式要求, 直接用 `display_text(...)[start..end]` 判断.
- 需要描述整串格式时, 再配合 `regex_is_match(...)`.

## 按键模式

默认模式是 `KEYBOARD`.

- 矩阵键盘题直接使用默认值即可.
- 独立按键题先调用 `key_mode(BTN)`.
- 也支持字符串形式, 比如 `key_mode("kbd")` 和 `key_mode("btn")`.

## LED 统计观察

- `watch_led_stats(L1, 40)`

`watch_led_stats` 是专门给 LED 评测准备的内建统计器. 它在仿真内核里逐步推进并统计 LED 变化次数, 变化频率, PWM 周期频率和占空比, 不需要在 Rhai 脚本里手写轮询循环.

- `watch_led_stats` 返回一个统计对象. 目前可直接读取:
- `stats.changes`
- `stats.change_frequency_hz`
- `stats.rising_edges`
- `stats.pwm_frequency_hz`
- `stats.duty_percent`
- `watch_led_stats` 同样会推进仿真时间. 对高频 PWM 建议至少观察多个周期, 并给结果留出范围余量.

例如 `led_flicker` 可以直接这样写:

```rhai
run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert(stats.changes >= 9 && stats.changes <= 11, "1 秒内 L1 线路变化次数约 10");
```

- 但是评测最好留有余量, 防止误差.

例如 `led_pwm` 可以这样写:

```rhai
run_ms(220);
let stats = watch_led_stats(L1, 40);
assert(stats.pwm_frequency_hz >= 950.0 && stats.pwm_frequency_hz <= 1050.0, "L1 PWM 频率约 1kHz");

assert(stats.duty_percent >= 8.0 && stats.duty_percent <= 12.0, "上电占空比约 10%");
```

## 数码管段码

- `seg_raw(1)`
- `seg_pattern(1)`
- `set_seg_decode(0x3F, "0")`
- `set_seg_blank(0x00)`

说明:

- `seg_raw(index)` 返回锁存到该位数码管上的原始字节.
- `seg_pattern(index)` 返回按 `!raw` 归一化后的段码模式, 更适合直接按 `0x3F` 这类常见段码表判断.
- `set_seg_decode(pattern, text)` 用于自定义 `display_text()` 的解码规则.
- `set_seg_blank(pattern)` 将某个模式视为留空.

默认已经内置了 `0-9 - P E L F H C` 的解码映射.

## 断言和调试

- `assert(cond, "message")`
- `assert_eq_str(actual, expected, "label")`
- `assert_eq_int(actual, expected, "label")`
- `print(anything)`

调试建议:

- 想在脚本中途打印, 用 `print(...)`.
- 想看固定时刻全量状态, 用 `dump` 子命令.
- 想看寄存器, 锁存器, LED, 段码, UART 等综合快照, 用 `print(snapshot_text())`.

## 示例

```rhai
run_ms(220);
assert(regex_is_match(display_text(30), "^\\s{7}0$"), "上电显示格式");
assert_eq_str(display_text()[0..7], "       ", "前7位空白");

set_key(S4, true);
run_ms(220);
let value = parse_int(display_text()[7..8]);
assert(value == 1, "显示稳定");
assert(led_on(L1), "L1 应点亮");

print(snapshot_text());
```
