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

- `RB2` `RB3` `RB4` `RD1`

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
- `set_voltage(RB2, 2.3)`
- `set_voltage("RB2", 2.3)`
- `uart_write("(F,?)")`
- `jumper_on(NET_SIG, SIG_OUT)`
- `jumper_off(NET_SIG, SIG_OUT)`
- `jumper_installed(NET_SIG, SIG_OUT)`

默认跳帽状态按开发板原理图处理. 例如 `NET_SIG` 和 `SIG_OUT` 在板内默认没有硬连, 所以仅仅 `set_frequency_hz(...)` 不会自动影响 `P3.4/T0`. 如果题目或评测需要把 NE555 输出接到单片机频率输入, 需要先显式调用 `jumper_on(NET_SIG, SIG_OUT)`.

## 输出观察

- `display_text()`
- `display_text(window_ms)`
- `snapshot_text()`
- `uart_take()`
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

## 按键模式

默认模式是 `KEYBOARD`.

- 矩阵键盘题直接使用默认值即可.
- 独立按键题先调用 `key_mode(BTN)`.
- 也支持字符串形式, 比如 `key_mode("kbd")` 和 `key_mode("btn")`.

## LED 频率观察

- `watch_led_changes(L1, 1000)`
- `watch_led_changes("L1", 1000)`
- `watch_led_frequency_hz(L1, 1000)`

`watch_led_changes` 是专门给 LED 评测准备的内建统计器. 它在仿真内核里逐步推进并统计状态翻转次数, 不需要在 Rhai 脚本里手写轮询循环.

例如 `led_flicker` 可以直接这样写:

```rhai
run_ms(20);
assert_eq_int(watch_led_changes(L1, 1000), 29, "1 秒内 L1 线路变化次数");
```

- 但是评测最好留有余量, 防止误差.

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
assert_eq_str(display_text(), "       0", "上电显示");

set_key(S4, true);
run_ms(220);
assert_eq_str(display_text(30), "       1", "显示稳定");
assert(led_on(L1), "L1 应点亮");

print(snapshot_text());
```
