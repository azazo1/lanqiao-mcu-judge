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

省略 `--hex` 时, 评测器会加载一个只会持续执行 `NOP` 的空程序. 这适合单独调试 Rhai 脚本, 输入注入, 波形捕获, 或仅依赖板级外设模型的脚本.

例如:

```bash
stcjudge run --script sample/ne555/judge/smoke.rhai
```

```bash
stcjudge repl
```

REPL 内置命令:

- `:help`
- `:quit`
- `:exit`

打开 `RUST_LOG=debug` 后, 评测器会输出 Rhai 脚本的逐语句执行进度, 包括:

- 当前脚本标签
- 步号
- 当前仿真时间 `sim_time_ns`
- 行号和列号
- 调用层级
- 当前语句所在源码行

其中 `sim_time_ns` 是当前语句对应的仿真时间, 单位为 `ns`. 当你想抓某一段协议, 中断, 刷新或显示异常附近的波形时, 可以先打开 `RUST_LOG=debug` 跑一遍脚本, 记下感兴趣日志附近的 `sim_time_ns`, 再把它换算或直接填给 `--wave-start` 和 `--wave-end`.

一个常见流程如下:

1. 先用 `RUST_LOG=debug` 正常执行脚本, 找到目标现象对应的 `sim_time_ns`.
2. 取一个稍早的起点和稍晚的终点, 例如目标点前后各留 `50us`, `200us`, `1ms` 之类的余量.
3. 重新执行同一条命令, 追加 `--wave-start ... --wave-end ... --wave-html ...`.
4. 打开导出的 HTML, 继续用查看器缩放和筛选信号.

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

reset 模式:

- `CPU_RESET` `RESET_CPU`
- `POWER_RESET` `RESET_POWER`

跳帽信号:

- `SIG_OUT`
- `NET_SIG`

`run_to(...)` 相关常量:

- 边沿: `UP` `DOWN` `FLIP`
- 数码管位: `D1` `D2` `D3` `D4` `D5` `D6` `D7` `D8`
- 数码管位别名: `SEG1` `SEG2` `SEG3` `SEG4` `SEG5` `SEG6` `SEG7` `SEG8`
- MCU 引脚: `P00` 到 `P57`, 也支持字符串形式 `P3.4`
- 协议线: `IIC_SCL` `IIC_SDA` `IIC_BUS_SCL` `IIC_BUS_SDA` `IIC_MASTER_SCL` `IIC_MASTER_SDA`
- 协议线: `IIC_SLAVE_SCL_LOW` `IIC_SLAVE_SDA_LOW` `ONEWIRE_MASTER` `ONEWIRE_BUS` `ONEWIRE_DEVICE_LOW`
- 协议线: `UART1_TX` `UART1_RX` `UART2_TX` `UART2_RX` `DS1302_CE` `DS1302_CLK` `DS1302_IO`
- 频率信号: `NET_SIG` 表示 NE555 原始输出线, `SIG_OUT` 表示板上的 `P3.4/SIG_OUT` 端, `NE555_SIG_OUT` 等价于 `NET_SIG`

这些常量可以直接传给脚本函数, 不需要再写成字符串.

## 执行控制

- `run_ms(ms)`
- `run_us(us)`
- `sim_time_ns()`
- `add_marker()`
- `add_marker(label)`
- `add_marker(time_ns)`
- `add_marker(time_ns, label)`
- `run_to(target, edge)`
- `run_to(target, edge, timeout_ns)`
- `run_to(predicate)`
- `run_to(predicate, timeout_ns)`
- `run_to_ns(target_ns)`
- `run_to_us(target_us)`
- `run_to_ms(target_ms)`
- `run_to_s(target_s)`

它们只推进仿真时间, 不等待真实时间.

`run_ms(...)` 和 `run_us(...)` 按精确仿真时基推进. 如果固件内部有显示刷新周期, 1s 测频窗口, 传感器采样节拍等逻辑, 在修改输入后要显式留出足够稳定时间, 不要假设几十毫秒内一定已经更新到最终结果.

`sim_time_ns()` 返回当前绝对仿真时间戳, 单位是 `ns`.

`add_marker(...)` 会向当前波形导出结果写入一个 marker. 不传参数时, 它会在当前 `sim_time_ns()` 位置写入一个匿名 marker.

`add_marker(label)` 会在当前时间写入一个带标签的 marker. `add_marker(time_ns)` 和 `add_marker(time_ns, label)` 则使用绝对 `ns` 时间戳, 语义和 `run_to_ns(...)` 一致.

如果当前没有开启 wave 导出, `add_marker(...)` 仍然可以安全调用, 但不会产生任何输出.

`run_to(target, edge)` 会持续推进仿真, 直到目标信号命中指定边沿, 返回这次一共推进了多少 `ns`.

`run_to(target, edge, timeout_ns)` 会在上面的基础上增加超时限制. 如果在 `timeout_ns` 内仍未命中目标边沿, 会直接报错.

- `UP` 表示 `false -> true`
- `DOWN` 表示 `true -> false`
- `FLIP` 表示任意翻转

常见示例:

```rhai
let dt0 = run_to(L1, FLIP);
let dt1 = run_to(P34, UP);
let dt2 = run_to("P3.4", "DOWN");
let dt3 = run_to(IIC_SCL, FLIP);
let dt4 = run_to(UART1_TX, DOWN);
let dt5 = run_to(ONEWIRE_BUS, UP);
let dt6 = run_to(NET_SIG, FLIP);
let dt7 = run_to(SIG_OUT, FLIP);
let dt8 = run_to(UART1_TX, FLIP, 200_000);
```

marker 示例:

```rhai
add_marker();
run_ms(10);
add_marker("after_boot");
add_marker(25_000_000, "sample_point");
```

`run_to(predicate)` 会持续推进仿真, 每推进一步就重新执行一次回调 `predicate`, 当其返回 `true` 时停止, 并返回这次一共推进了多少 `ns`.

`run_to(predicate, timeout_ns)` 则会额外增加超时限制.

回调应当返回布尔值. 常见写法例如:

```rhai
let dt0 = run_to(|| led_on(L1));
let target_ns = sim_time_ns() + 20_000;
let dt1 = run_to(|| sim_time_ns() >= target_ns, 30_000);
let dt2 = run_to(|| display_text() == "000", 2_000_000);
```

对 `delay` 这类纯时序题, 可以先等待上电初始化结束, 再测后续步进:

```rhai
fn all_off() {
    let led = 1;
    while led <= 8 {
        if led_on(led) {
            return false;
        }
        led += 1;
    }
    true
}

let startup_ns = run_to(|| all_off(), 2_000_000);
assert_in(startup_ns, 0..=200_000, "上电后应先清空初始 LED 输出");

let dt0 = run_to(L1, UP, 20_000_000);
let dt1 = run_to(L2, UP, 20_000_000);
assert_in(dt0, 4_500_000..=5_500_000, "step0 Delay5ms 约为 5ms");
assert_in(dt1, 4_500_000..=5_500_000, "step1 Delay5ms 约为 5ms");
```

对类似超声波, 温度, 电压这类题目, 推荐优先使用稳定显示和按位数值提取, 不要直接依赖原始段码:

```rhai
run_ms(220);
assert_eq(display_text(30)[0..1], "L", "默认距离页");
assert_eq(display_number(4, 8, 30), 0, "默认距离");

tap_key(S4, 80);
assert_eq(display_text(30)[0..1], "P", "切到音速页");
assert_eq(display_number(6, 8, 30), 340, "默认音速");
```

注意, 回调内部也可以调用其他脚本接口, 包括读取显示, LED, 串口, 甚至继续推进仿真时间. `run_to` 返回的耗时会按真实推进后的仿真时间计算.

但回调应尽量保持轻量:

- 更适合只做简单布尔条件判断.
- 不要在回调里反复做正则匹配, 多次字符串切片, 多次数值解析, 或其他明显偏重的逻辑.
- 如果目标只是等待按键后显示稳定, 优先用 `tap_key(...)` 自带的释放后等待, 再配合普通的 `display_text(...)` 或 `display_number(...)` 断言, 不要把整套显示解析都塞进 `run_to(predicate)`.

不推荐这样写:

```rhai
run_to(
    || {
        let text = screen_now();
        regex_is_match(text, "^\\d{2}-\\d-\\d{3}$")
            && parse_int(text[0..2]) == vv_expected
            && parse_int(text[3..4]) == r_expected
            && parse_int(text[5..8]) == eee_expected
    },
    250_000_000
);
```

这类写法每一步都要重复取显示, 做正则, 切片, 解析数值, 在长时间等待场景下会明显拖慢脚本执行.

更推荐拆开处理:

1. 如果只是等待按键释放后的稳定态, 直接依赖 `tap_key(...)` 内置等待.
2. 如果题目本身存在 `100ms`, `500ms`, `1s` 之类的业务刷新节拍, 显式 `run_ms(...)` 留出对应余量.
3. 等显示稳定后, 再在普通断言里做字符串切片, 正则或数值解析.

例如:

```rhai
tap_key(S4, 80);
run_ms(120);

let text = display_text(30);
assert(regex_is_match(text, "^\\d{2}-\\d-\\d{3}$"), "显示格式");
assert_eq(parse_int(text[0..2]), vv_expected, "VV");
assert_eq(parse_int(text[3..4]), r_expected, "R");
assert_eq(parse_int(text[5..8]), eee_expected, "EEE");
```

`run_to_ns/us/ms/s(...)` 的参数是绝对仿真时间戳, 不是相对等待时长. 它们同样返回本次推进的时间:

- `run_to_ns(...)` 返回 `ns`, 类型为整数
- `run_to_us(...)` 返回 `us`, 类型为浮点
- `run_to_ms(...)` 返回 `ms`, 类型为浮点
- `run_to_s(...)` 返回 `s`, 类型为浮点

例如:

```rhai
let dt_ns = run_to_ns(1_000);
let dt_us = run_to_us(250);
let dt_ms = run_to_ms(1.5);
let dt_s = run_to_s(2);
```

注意:

- 不带 `timeout_ns` 的 `run_to(...)` 如果条件永远不满足, 脚本会一直运行下去.
- `run_to(...)` 的命中精度取决于仿真步进. 返回值是首次观测到目标边沿或条件成立时, 相对于调用点累计推进的时间.

## 状态导入导出

- `export_persistent_state()`
- `load_persistent_state(text)`
- `reset()`
- `reset(mode)`

`export_persistent_state()` 返回当前非易失外设状态的序列化字符串. 当前覆盖:

- `DS18B20` 的 ROM 和 EEPROM 配置.
- `DS1302` 的时钟寄存器, 写保护, trickle charge 和 31 字节 RAM.
- `AT24C02` 的全部 256 字节存储内容.

`load_persistent_state(text)` 用同版本评测器导出的字符串覆盖当前非易失状态. 它不会自动恢复 MCU 寄存器, 内部 RAM, 数码管采样缓存, LED 当前态, UART 队列等易失运行态. 如果固件把外设内容缓存到了 RAM, 脚本里通常还需要额外调用 `reset()` 或触发固件自己的重新读取流程.

`reset()` 等价于 `reset(POWER_RESET)`. 它会重建 MCU 和板级运行环境, 语义上等价于重新上电. 它会清空外设和板级的易失运行态, 但会保留非易失外设状态, 并保留当前脚本注入条件, 包括:

- 当前按键模式和按下状态.
- 当前跳帽连接关系.
- 当前模拟电压输入.
- `set_temperature_c(...)`, `set_distance_cm(...)`, `set_frequency_hz(...)`, `set_ds18b20_parasite_power(...)` 注入的环境条件.

`reset(CPU_RESET)` 只复位 MCU 自身状态, 包括 PC, SFR, 定时器, UART, MOVX RAM 和端口锁存器. 它不会重建外设实例, 因而会保留外设和板级当前的易失运行态, 例如:

- `AT24C02` 当前的 busy 写周期和地址指针.
- `PCF8591` 当前的 DAC 输出值和通道选择.
- 板级锁存器当前输出, 以及由此带来的 LED, 继电器, 蜂鸣器和数码管当前显示状态.

`reset(POWER_RESET)` 则会把上面这些板级和外设易失态一并清空, 只留下非易失内容和脚本注入条件.

持久状态字符串只保证在同版本评测器内部自洽, 不建议跨版本长期保存或手工构造.

## 输入注入

- `set_key(S4, true)`
- `set_key("S4", true)`
- `tap_key(S4, 50)`
- `key_mode(BTN)`
- `key_mode("kbd")`
- `set_rtc(23, 59, 50)`
- `set_temperature_c(25)`
- `set_temperature_c(25.9375)`
- `set_ds18b20_rom("280123456789AB")`
- `set_ds18b20_parasite_power(true)`
- `set_distance_cm(35)`
- `set_frequency_hz(2200)`
- `set_voltage(AIN3, 2.3)`
- `set_voltage("AIN1", 2.3)`
- `uart_write("(F,?)")`
- `jumper_on(NET_SIG, SIG_OUT)`
- `jumper_off(NET_SIG, SIG_OUT)`
- `jumper_installed(NET_SIG, SIG_OUT)`

默认跳帽状态按开发板原理图处理. 例如 `NET_SIG` 和 `SIG_OUT` 在板内默认没有硬连, 所以仅仅 `set_frequency_hz(...)` 不会自动影响 `P3.4/T0`. 如果题目或评测需要把 NE555 输出接到单片机频率输入, 需要先显式调用 `jumper_on(NET_SIG, SIG_OUT)`.

`tap_key(...)` 会自动执行一次完整的按下和释放流程:

- 先按下目标按键.
- 推进 `hold_ms`.
- 再释放按键.
- 释放后额外再推进 `30ms`.

如果只是为了等待按键释放后的稳定态, 一般不需要在 `tap_key(...)` 后面再手写一段额外的 `settle`.

但这不等于可以省掉题目本身的业务刷新等待. 例如显示任务本身每 `100ms` 才更新一次, 那么在一串按键操作完成后, 仍然可能需要额外 `run_ms(...)` 去等待显示刷新, 这部分等待应按题目自身节拍来决定.

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
- `eeprom_byte(addr)`
- `relay_on()`
- `buzzer_on()`
- `motor_on()`
- `led_on(1)`
- `led_on(L1)`

`display_text()` 返回当前已经采样到的稳定显示结果. 当前实现不是去读某一个瞬时扫描相位, 而是维护每一位最近一次被锁存到的段码, 所以对于静态显示题目通常已经足够稳定.

这里返回的是"解码后的文本", 不是固定 8 个字符:

- 每个物理数码管位通常解码成 1 个字符.
- 如果这一位带小数点, 可能会解码成 2 个字符, 例如 `5.`.
- 如果只有小数点点亮, 会解码成 `.`.
- 末尾空白位会被裁掉, 所以返回字符串长度不一定等于 8.

`display_text(window_ms)` 会推进仿真时间, 在给定窗口内持续观察显示内容:

- 如果整个窗口内显示文本保持不变, 返回该文本.
- 如果窗口内文本发生变化, 直接报错.

这样更适合判断一段时间内显示是否稳定, 而不是只看某个瞬间.

`display_number()` 和 `display_number(window_ms)` 会从当前显示文本里提取唯一的数值. 如果显示内容里没有数字, 或者同时出现了多个数字, 会直接报错.

- 如果提取到的是纯整数, 返回整数.
- 如果提取到的数字里包含小数点, 返回浮点数.

如果一屏里同时存在多个数字片段, 可以改用 `display_number(start, end)` 或 `display_number(start, end, window_ms)`, 在指定的数码管位范围内提取数字. 位号和 `seg_pattern(1)` 一样, 都是从左到右按 `1..=8` 计数.

带 `window_ms` 的版本会先复用 `display_text(window_ms)` 的整屏稳定性检查, 再读取指定范围的数值. 如果题目本身是扫描显示, 但你只关心其中几位, 有时直接用不带窗口的范围版本会更稳.

如果显示里本来就混有空白位, 固定符号位, 或者像温度值 + 等级值这种分段内容, 优先按物理数码管位范围读取, 例如 `display_number(1, 6)` 和 `display_number(8, 8)`. 这样在显示带小数点时, 不会被 `display_text()` 的可变字符长度干扰.
如果 `display_text(window_ms)` 恰好跨过一次显示刷新边缘, 也可能把中间过渡态识别成变化. 这类场景可以先额外 `run_ms(20)` 或 `run_ms(30)`, 再直接读取一次 `display_text()`.

`display_number(...)` 接受前导零, 但返回值只保留数值本身. 如果题目要求精确判断位宽, 前导零, 空白位, 固定符号位, 请直接对 `display_text(...)` 的结果做字符串切片和正则判断.

这更适合超声波, 温度, 电压这类量测题, 可以直接写布尔表达式判断范围, 避免按整串字符串做数值断言.

`da_value()` 返回当前 DA 输出的原始数值, 范围是 `0..=255`. 对 `PCF8591` 这类 AD/DA 题, 可以直接用它验证按键调节后的 DA 输出是否正确.

`eeprom_byte(addr)` 返回当前 `AT24C02` 指定地址中的原始字节, 范围是 `0..=255`. 对需要验证 EEPROM 块扫描, 指针回绕, 持久化恢复的题目, 可以直接读取指定地址, 不必只靠数码管结果反推内部状态.

`uart_take()` 返回当前已经发出的串口文本, 并清空内部发送队列. 如果要确认某次串口输出已经被完整消费, 可以连续调用两次, 第二次应返回空字符串.

串口题常见写法:

```rhai
uart_write("00012");
run_ms(220);
assert_eq(uart_take(), "13", "串口应返回原值加 1");
assert_eq(uart_take(), "", "读取后串口缓冲应为空");
let text = display_text(30);
assert_eq(text[0..3], "   ", "前 3 位保持空白");
assert_eq(text[3..8], "00012", "右 5 位补零显示原值");
```

## Rhai 字符串切片和正则

- `regex_is_match(text, pattern)`
- `regex_match(text, pattern)`

Rhai 自带字符串切片语法, 可以直接写 `text[0..5]`. 这里的范围是 `start..end`, 也就是 0 基, 右边界不包含在结果里.

Rhai 也自带数值解析函数, 例如:

- `parse_int("123")`
- `parse_float("123.45")`
- `parse_int("ff", 16)`

推荐写法:

- 数值部分如果对应固定物理数码管位, 优先用 `display_number(start, end)` 或 `display_number(start, end, window_ms)`.
- 数值部分如果确实要按字符串格式判断, 再用 `display_text(...)[start..end]` 配合 `parse_int(...)` 或 `parse_float(...)`.
- 固定字符, 空白位, 前导零, 分隔符等格式要求, 直接用 `display_text(...)[start..end]` 判断.
- 需要描述整串格式时, 再配合 `regex_is_match(...)`.
- 不要先看当前 `hex` 的输出再反推 `expect`, 应先根据题意, 源码, 手册推导出应有结果, 再写断言.

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
assert_in(stats.changes, 9..=11, "1 秒内 L1 线路变化次数约 10");
```

- 但是评测最好留有余量, 防止误差.

例如 `led_pwm` 可以这样写:

```rhai
run_ms(220);
let stats = watch_led_stats(L1, 40);
assert_in(stats.pwm_frequency_hz, 950..=1050, "L1 PWM 频率约 1kHz");

assert_in(stats.duty_percent, 8..=12, "上电占空比约 10%");
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
- `assert_eq(actual, expected, "label")`
- `assert_in(actual, 10..=12, "label")`
- `print(anything)`

`assert_eq(...)` 要求 `actual` 和 `expected` 是同类型. 适合字符串, 整数, 浮点, 布尔等直接相等比较. 失败时会同时打印 `expected` 和 `actual`.
`assert_in(...)` 适合整数和浮点数的区间判断. 目前使用 Rhai 的整数 range 语法, 支持 `a..b` 和 `a..=b`. 对浮点实际值会按对应的整数边界比较. 失败时会同时打印期望区间和实际值.

调试建议:

- 想在脚本中途打印, 用 `print(...)`.
- 想看固定时刻全量状态, 用 `dump` 子命令.
- 想看寄存器, 锁存器, LED, 段码, UART 等综合快照, 用 `print(snapshot_text())`.

## 示例

```rhai
run_ms(220);
let text = display_text(30);
assert_eq(text[0..7], "       ", "前7位空白");
assert_eq(parse_int(text[7..8]), 0, "上电末位数值");

set_key(S4, true);
run_ms(220);
let value = parse_int(display_text()[7..8]);
assert_eq(value, 1, "显示稳定");
assert(led_on(L1), "L1 应点亮");

set_temperature_c(25.9375);
run_ms(700);
assert_eq(display_number(1, 6), 25.500, "9bit 温度显示");
assert_eq(display_number(8, 8), 0, "精度等级");

print(snapshot_text());
```
