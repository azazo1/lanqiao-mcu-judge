# Rhai 评测脚本手册

## 目录约定

- 每个题目的评测脚本放在 `samples/xxx/judge/`.
- 推荐把冒烟脚本命名为 `smoke.rhai`.

## 运行方式

本文默认已经拿到编译后的可执行文件 `stcjudge`. 例如执行 `cargo build --release` 后, 二进制路径为 `target/release/stcjudge`.

脚本文件:

```bash
stcjudge run --hex samples/key_seg/prj/Objects/key_seg.hex --script samples/key_seg/judge/smoke.rhai
```

标准输入:

```bash
stcjudge run --hex samples/key_seg/prj/Objects/key_seg.hex --stdin < samples/key_seg/judge/smoke.rhai
```

交互式 REPL:

```bash
stcjudge repl --hex samples/key_seg/prj/Objects/key_seg.hex
```

脚本逐语句 tracing:

```bash
RUST_LOG=debug stcjudge run --hex samples/key_seg/prj/Objects/key_seg.hex --script samples/key_seg/judge/smoke.rhai
```

省略 `--hex` 时, 评测器会加载一个只会持续执行 `NOP` 的空程序. 这适合单独调试 Rhai 脚本, 输入注入, 波形捕获, 或仅依赖板级外设模型的脚本.

例如:

```bash
stcjudge run --script samples/ne555/judge/smoke.rhai
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
- `run_ms_bounded(wall_ms, sim_ms)`
- `run_us_bounded(wall_us, sim_us)`
- `sim_time_ns()`
- `add_marker()`
- `add_marker(label)`
- `add_marker(time_ns)`
- `add_marker(time_ns, label)`
- `run_to(target, edge)`
- `run_to(target, edge, timeout_ns)`
- `run_to(predicate)`
- `run_to(predicate, timeout_ns)`
- `run_to_state(target, expected)`
- `run_to_state(target, expected, timeout_ns)`
- `run_to_event(track)`
- `run_to_event(track, timeout_ns)`
- `run_to_ns(target_ns)`
- `run_to_us(target_us)`
- `run_to_ms(target_ms)`
- `run_to_s(target_s)`

它们只推进仿真时间, 不等待真实时间.

`run_ms(...)` 和 `run_us(...)` 按精确仿真时基推进. 如果固件内部有显示刷新周期, 1s 测频窗口, 传感器采样节拍等逻辑, 在修改输入后要显式留出足够稳定时间, 不要假设几十毫秒内一定已经更新到最终结果.

`run_ms_bounded(wall_ms, sim_ms)` 和 `run_us_bounded(wall_us, sim_us)` 会持续推进仿真, 直到仿真时间达到上限, 或者真实执行时间达到上限. 返回值是一个 map:

- `requested_sim_time_ns`: 请求推进的仿真时间, 单位 `ns`
- `elapsed_sim_time_ns`: 实际推进的仿真时间, 单位 `ns`
- `elapsed_wall_time_ns`: 实际消耗的真实时间, 单位 `ns`
- `hit_sim_limit`: 是否命中仿真时间上限
- `hit_wall_limit`: 是否命中真实时间上限

例如:

```rhai
let stats = run_us_bounded(5_000, 20_000);
if stats.hit_wall_limit {
    print("本轮仿真先触发真实时间上限");
}
```

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

`run_to_state(target, expected)` 会持续推进仿真, 直到某个底层状态等于目标值, 并返回这次一共推进了多少 `ns`.

`run_to_state(target, expected, timeout_ns)` 则会额外增加超时限制.

`run_to_state(...)` 适合把"等待状态出现"和"断言状态语义"拆开写. 对扫描显示, 闪烁字段, 锁存器, 引脚电平这类场景, 它通常比 `run_to(predicate)` 更快, 也更容易定位失败点.

对数码管显示尤其要注意: 不要把 `run_to_state("seg.text", expected, ...)` 当成显示内容断言器来用. 如果目标是拿到一屏已经稳定下来的显示内容, 更推荐直接用 `display_text(20)` 或 `display_text(30)` 这类方式, 让脚本等待到整屏连续稳定一段时间后再读取. 如果你测的是某个闪烁相位, 某一位重新点亮的时刻, 或者显示开始响应的瞬间, 再用 `run_to_state("seg.dN.visible", ...)` / `run_to_event("seg.change", ...)` 对齐时机. 这样一旦显示错了, 脚本会直接报出实际值, 不会被"一直等不到期望字符串"掩盖.

常见 target 例子:

- 布尔 target: `pin.p3.4`, `latch.p2.1`, `board.effective.com.0`, `seg.d3.scan`, `seg.d3.visible`
- 整数 target: `pin.p3`, `latch.p3`, `board.effective.ctrl`, `seg.d8.pattern`, `seg.d8.raw`
- 文本 target: `seg.text`, `seg.d8.text`

`run_to_event(track)` 会持续推进仿真, 直到捕获到下一条 wave event, 并返回一个 map:

- `track`: 规范轨道 id, 例如 `event.uart1`
- `time_ns`: 事件发生时刻
- `elapsed_ns`: 这次等待推进的时长
- `label`: 事件标签, 例如 `RX 0x41`
- `detail`: 附加细节, 例如 `bits=8`

`run_to_event(track, timeout_ns)` 则会额外增加超时限制.

`run_to_event(...)` 的职责也应当保持轻量, 只负责等待某类事件真正发生, 不要把复杂语义塞进等待条件里. 更推荐的写法是: 先等事件出现, 再立刻读取当前状态并做普通断言. 这样如果固件显示错了, 你能直接看到错误现场, 而不是只拿到一个超时.

常见 track 例子:

- `event.cpu`
- `event.i2c`
- `event.onewire`
- `event.uart1`
- `event.uart2`
- `event.adc_dac`
- `event.ds1302`
- `event.seg.change`
- `event.seg.d1.change` 到 `event.seg.d8.change`
- `uart1`, `uart2`, `rtc` 这类简写也可以
- `seg.change`, `seg.d1.change` 到 `seg.d8.change` 这类简写也可以

其中 `seg.change` 表示整屏数码管状态发生了一次有效变化, `seg.dN.change` 表示第 `N` 位数码管状态发生了一次有效变化. 这里的 "有效变化" 是按底层段码和可见状态判断的, 不是按 `display_text()` 提取后的字符串判断. 因此同样的扫描刷新不会重复触发, 只有这一位的 `seen` / `segments` 真正变化时才会产生事件.

对扫描显示要区分两类场景:

- 如果题目里的某一位会闪烁, 优先用 `run_to_state("seg.dN.visible", true, ...)` 或 `run_to_state("seg.dN.visible", false, ...)` 抓可见/隐藏相位, 然后在对应时刻读取显示并断言.
- 如果是按键, 串口, 传感器或其他输入动作之后, 想直接断言最终整屏显示, 更推荐先完成输入动作, 再用 `display_text(20)` 或 `display_text(30)` 读取稳定后的结果.
- 如果你关心的是"显示从什么时候开始动了", 可以先用 `run_to_event("seg.change", ...)` 抓第一次有效段码变化.

要注意, `seg.change` 和 `seg.dN.change` 捕获到的是 "某一位发生了有效变化" 这件事本身, 不保证在该时刻整屏已经全部刷新完成. 动态扫描下, 你可能先观察到其中一位变化, 其余位还在后续扫描周期里继续加载. 因此它们更适合做"时机对齐"而不是"整屏稳定断言". 实际写脚本时, 更稳妥的做法通常是:

1. 先用 `run_to_event("seg.change", ...)` 抓到第一次有效变化.
2. 如果你只是想读稳定整屏, 直接调用 `display_text(20)` 或 `display_text(30)`.
3. 如果你测的是更细的相位, 再结合 `display_text()`, `seg.dN.text`, `seg.dN.raw` 做断言.

这样不会把脚本绑死在某一位必须发生变化上. 例如显示从 `10` 变到 `20` 时, 真正变化的可能只有十位, 如果你硬等 `seg.d2.change` 之类的特定位变化, 反而可能把正确实现误判成超时. `seg.dN.change` 更适合你明确知道哪一位一定会变, 并且确实想测这一位本身的响应时刻时再使用. 如果只是要看整屏最终显示, `display_text(ms)` 一般更直接.

如果是闪烁字段重新显示, 尤其是一个字段横跨两位或更多位时, 可以在 `run_to_state("seg.dN.visible", true, ...)` 命中后直接调用 `display_text(20)` 或 `display_text(30)` 读取该相位稳定下来的整屏. 原因是 `visible` 只说明目标位已经重新点亮, 其余同字段位可能还在这一轮动态扫描里陆续刷新.

`run_to(...)`, `run_to_state(...)`, `run_to_event(...)` 带 `timeout_ns` 时, 超时语义是直接报运行时错误, 不会返回一个特殊错误值给脚本继续判断. 在 judge 脚本里, 这通常就意味着当前评测脚本直接失败. 当前实现按 "`elapsed <= timeout_ns` 成功, `elapsed > timeout_ns` 失败" 处理, 也就是恰好在 `timeout_ns` 时刻命中仍算成功. 因此如果题目要求某个显示或外设必须在 `100ms` 内响应, 而你只关心 "是否超过 100ms", 那么最直接的写法就是把 `timeout_ns` 直接写成 `100_000_000`.

例如:

```rhai
tap_key(S4, 80);
run_to_event("seg.change", 100_000_000);
run_ms(20);

tap_key(S5, 80);
run_to_state("seg.d3.visible", true, 100_000_000);
```

上面这种写法里, 只要目标没有在 `100ms` 内命中, 脚本就会直接报超时错误, 不需要再手动判断返回值.

返回值本身仍然很有用. 它们给出的就是从调用点到目标命中的仿真延迟, 适合在以下场景继续使用:

- 你想记录或断言真实响应时间, 而不只是做一个超时上限判断.
- 你给的 `timeout_ns` 比题目门槛稍宽, 既想避免长时间卡住, 又想单独校验更严格的实时性指标.
- 你想比较不同输入路径, 不同状态页, 不同外设之间的响应延迟差异.

例如:

```rhai
tap_key(S4, 80);
let event = run_to_event("seg.change", 120_000_000);
assert_in(event.elapsed_ns, 0..=100_000_000, "显示响应: 启动时延超出 100ms");
run_ms(20);

tap_key(S5, 80);
let ready_ns = run_to_state("seg.d3.visible", true, 120_000_000);
assert_in(ready_ns, 0..=100_000_000, "闪烁位显示: 恢复时延超出 100ms");
```

回调应当返回布尔值. 在 judge 编写约定里, `run_to(predicate)` 只适合最基础的条件判断, 可以把它理解成一个轻量轮询器, 而不是通用断言容器. 常见写法例如:

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
assert_in(startup_ns, 0..=200_000, "上电初始化: LED 清空时延超出范围");

let dt0 = run_to(L1, UP, 20_000_000);
let dt1 = run_to(L2, UP, 20_000_000);
assert_in(dt0, 4_500_000..=5_500_000, "step0 Delay5ms: 耗时超出范围");
assert_in(dt1, 4_500_000..=5_500_000, "step1 Delay5ms: 耗时超出范围");
```

对类似超声波, 温度, 电压这类题目, 推荐优先使用稳定显示和按位数值提取, 不要直接依赖原始段码:

```rhai
run_ms(220);
assert_eq(display_text(30)[0..1], "L", "默认距离页: 页面前缀错误");
assert_eq(display_number(4, 8, 30), 0, "默认距离页: 距离数值错误");

tap_key(S4, 80);
assert_eq(display_text(30)[0..1], "P", "音速页: 页面前缀错误");
assert_eq(display_number(6, 8, 30), 340, "音速页: 音速数值错误");
```

从引擎能力上说, 回调里确实还能调用别的脚本接口. 但在本项目 judge 编写约定里, `run_to(predicate)` 默认只用于最基础的布尔判断, 不要把复杂语义分析塞进回调.

推荐用途:

- 等单个状态翻转, 例如 `led_on(L1)`.
- 等绝对时间到达, 例如 `sim_time_ns() >= target_ns`.
- 等一个非常简单的显示条件成立, 例如 `display_text() == "000"`.

不要这样用:

- 在回调里反复做正则匹配.
- 在回调里做多次字符串切片.
- 在回调里做多次数值解析.
- 在回调里组合多个显示字段的语义判断.
- 在回调里继续调用 `run_ms(...)` `run_us(...)` 等推进时间.
- 把按键后的整套"等待稳定 + 解析显示 + 断言数值"都塞进一个 `run_to(predicate)`.

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

这类写法每一步都要重复取显示, 做正则, 切片, 解析数值, 在长时间等待场景下会明显拖慢脚本执行, 也会让失败点变得不易追踪.

更推荐拆开处理:

1. 如果只是等待按键释放后的稳定态, 直接依赖 `tap_key(...)` 内置等待.
2. 如果题目本身存在 `100ms`, `500ms`, `1s` 之类的业务刷新节拍, 显式 `run_ms(...)` 留出对应余量.
3. 如果是扫描显示或闪烁字段, 优先用 `run_to_state(...)` 等底层状态就绪, 再在普通断言里做字符串切片, 正则或数值解析.
   不要直接写 `run_to_state("seg.text", expected, ...)` 来等待显示内容正确.
4. 等显示稳定后, 再在普通断言里做字符串切片, 正则或数值解析.

像 `DS1302` 这种设置态会按约 `500ms` 闪烁的题, 更推荐先等待当前字段重新可见, 再做普通断言:

```rhai
tap_key(S4, 80);
run_to_state("seg.d3.visible", true, 1_200_000_000);

let text = display_text();
assert_regex(text, "^  \\d{2}\\.\\d{2}\\.\\d{2}$", "时间页: 显示格式错误");
assert_eq(parse_int(text[2..4]), 23, "时间页: 小时显示错误");
assert_eq(parse_int(text[5..7]), 59, "时间页: 分钟显示错误");
assert_eq(parse_int(text[8..10]), 58, "时间页: 秒显示错误");
```

不要把上面的流程改写成 `run_to_state("seg.text", "  23.59.58", ...)`. 如果固件实际显示成别的值, 这种写法通常只会报超时, 很难第一时间看出它到底显示错成了什么.

例如:

```rhai
tap_key(S4, 80);
run_ms(120);

let text = display_text(30);
assert_regex(text, "^\\d{2}-\\d-\\d{3}$", "数据显示页: 显示格式错误");
assert_eq(parse_int(text[0..2]), vv_expected, "数据显示页: VV 显示错误");
assert_eq(parse_int(text[3..4]), r_expected, "数据显示页: R 显示错误");
assert_eq(parse_int(text[5..8]), eee_expected, "数据显示页: EEE 显示错误");
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

无论是 `reset(CPU_RESET)` 还是 `reset(POWER_RESET)`, 当前 `sim_time_ns()` 都会保持不变, 不会回退到 `0`. 如果你需要基于 reset 前后计算耗时, 可以直接继续使用绝对时间差.

持久状态字符串只保证在同版本评测器内部自洽, 不建议跨版本长期保存或手工构造.

## 输入注入

- `set_key(S4, true)`
- `set_key("S4", true)`
- `tap_key(S4, 50)`
- `key_mode(BTN)`
- `key_mode("kbd")`
- `set_rtc(23, 59, 50)`
- `set_rtc(#{ hour: 23, minute: 59, second: 50, running: false })`
- `set_temperature_c(25)`
- `set_temperature_c(25.9375)`
- `set_ds18b20_rom("280123456789AB")`
- `set_ds18b20_parasite_power(true)`
- `set_distance_cm(35)`
- `set_frequency_hz(2200)`
- `set_voltage(AIN3, 2.3)`
- `set_voltage("AIN1", 2.3)`
- `set_eeprom(0x10, 0xAB)`
- `set_eeprom(0x20, [1, 2, 3])`
- `peek_data(0x30)`
- `peek_iram(0x30)`
- `peek_idata(0x30)`
- `poke_data(0x30, 0x5A)`
- `poke_iram(0x30, 0x5A)`
- `poke_idata(0x30, 0x5A)`
- `peek_sfr(0x90)`
- `peek_sfr_latch(0x90)`
- `poke_sfr(0x90, 0x55)`
- `peek_pdata(0x32)`
- `poke_pdata(0x32, 0x77)`
- `peek_xdata(0x1234)`
- `poke_xdata(0x1234, 0xAB)`
- `uart_config(8, 9600, 1, "none")`
- `uart_write("(F,?)")`
- `uart1_write("(F,?)")`
- `uart2_write("(F,?)")`
- `uart1_write_raw([0x055, 0x141])`
- `uart2_write_raw([0x055, 0x141])`
- `jumper_on(NET_SIG, SIG_OUT)`
- `jumper_off(NET_SIG, SIG_OUT)`
- `jumper_installed(NET_SIG, SIG_OUT)`

默认跳帽状态按开发板原理图处理. 例如 `NET_SIG` 和 `SIG_OUT` 在板内默认没有硬连, 所以仅仅 `set_frequency_hz(...)` 不会自动影响 `P3.4/T0`. 如果题目或评测需要把 NE555 输出接到单片机频率输入, 需要先显式调用 `jumper_on(NET_SIG, SIG_OUT)`.

`set_frequency_hz(...)` 的语义是: 从 `当前仿真时刻` 立即切换到指定频率, 并保持相位连续. 它不是 "把整个仿真从 0ns 开始都当作这个频率".

`tap_key(...)` 会自动执行一次完整的按下和释放流程:

- 先按下目标按键.
- 推进 `hold_ms`.
- 再释放按键.
- 释放后额外再推进 `30ms`.

如果只是为了等待按键释放后的稳定态, 一般不需要在 `tap_key(...)` 后面再手写一段额外的 `settle`.

但这不等于可以省掉题目本身的业务刷新等待. 例如显示任务本身每 `100ms` 才更新一次, 那么在一串按键操作完成后, 仍然可能需要额外 `run_ms(...)` 去等待显示刷新, 这部分等待应按题目自身节拍来决定.

`set_rtc(23, 59, 50)` 仍然是便捷写法, 只会修改时, 分, 秒, 并把亚秒进度清零.

`set_rtc(#{ ... })` 会按状态方式设置 RTC. 当前支持这些字段:

- `hour`, `minute`, `second`
- `year`, `month`, `date`, `day_of_week`, `weekday`
- `running`, `halted`
- `hour_mode`, `hour_mode_12`
- `write_protect`
- `trickle_charge`

未提供的字段会保持当前值不变, 只有传入的字段才会被设置.

`running: false` 等价于 `halted: true`. `hour_mode` 接受 `12`, `24`, `12h`, `24h`. 如果一次调用里修改了任意时间或日期字段, 评测器会把 RTC 的亚秒进度清零.

`peek_data(addr)` 和 `poke_data(addr, value)` 直接访问内部 RAM 的低 `128` 字节, 也就是 `C51` 里通常说的 `data` 区. 地址范围必须在 `0..=127`.

`peek_iram(addr)` 和 `poke_iram(addr, value)` 直接访问整块内部 RAM, 地址范围是 `0..=255`. `peek_idata(...)` 和 `poke_idata(...)` 是同一块存储的别名, 只是名字上更贴近 `idata`.

`peek_sfr(addr)` 读取当前 `SFR` 的输入视图. 对端口寄存器来说, 它更接近 CPU 指令实际读到的引脚电平. `peek_sfr_latch(addr)` 则读取端口锁存值, 更适合看程序最后写进去了什么. `poke_sfr(addr, value)` 会按仿真器当前的 `SFR` 写入语义直接更新寄存器, 地址范围必须在 `0x80..=0xFF`.

`peek_pdata(addr)` 和 `poke_pdata(addr, value)` 访问 `pdata` 视图. 当前实现把它当作 `xdata` 低 `256` 字节的页内别名, 地址范围是 `0..=255`.

`peek_xdata(addr)` 和 `poke_xdata(addr, value)` 直接读写线性的 `xdata` 空间, 地址范围是 `0..=65535`.

这组接口适合两类场景:

- 定位 `UART`, 中断, 参数解析, 缓冲区等问题时, 直接观察关键内存和寄存器值.
- 在确实需要时, 直接布置 `data`, `idata`, `pdata`, `SFR`, `xdata` 场景, 避开冗长的前置交互.

例如:

```rhai
poke_data(0x30, 0x12);
assert_eq(peek_idata(0x30), 0x12, "内部 RAM: 回读错误");

poke_sfr(0x90, 0x55);
assert_eq(peek_sfr_latch(0x90), 0x55, "P1 锁存器: 回读错误");

poke_pdata(0x32, 0x77);
assert_eq(peek_xdata(0x32), 0x77, "页内 XDATA: 回读错误");
```

如果需要根据 `Keil` 编译产物里的符号和段信息定位这些地址, 可参考 [peek-poke-debug.md](peek-poke-debug.md).

## 输出观察

- `display_text()`
- `display_text(window_ms)`
- `display_number()`
- `display_number(window_ms)`
- `display_number(start, end)`
- `display_number(start, end, window_ms)`
- `snapshot_text()`
- `uart_take()`
- `uart_take(idle_ms)`
- `uart_take_raw()`
- `uart_take_raw(idle_ms)`
- `uart1_take()`
- `uart1_take(idle_ms)`
- `uart1_take_raw()`
- `uart1_take_raw(idle_ms)`
- `uart2_take()`
- `uart2_take(idle_ms)`
- `uart2_take_raw()`
- `uart2_take_raw(idle_ms)`
- `uart_peek()`
- `uart_peek(idle_ms)`
- `uart_peek_raw()`
- `uart_peek_raw(idle_ms)`
- `uart1_peek()`
- `uart1_peek(idle_ms)`
- `uart1_peek_raw()`
- `uart1_peek_raw(idle_ms)`
- `uart2_peek()`
- `uart2_peek(idle_ms)`
- `uart2_peek_raw()`
- `uart2_peek_raw(idle_ms)`
- `da_value()`
- `eeprom_byte(addr)`
- `relay_on()`
- `buzzer_on()`
- `motor_on()`
- `led_on(1)`
- `led_on(L1)`

`display_text()` 返回当前已经采样到的稳定显示结果. 当前实现不是去读某一个瞬时扫描相位, 而是维护每一位最近一次被锁存到的段码, 但如果某一位连续 100ms 没有再次动态扫描更新, 这一位会自动熄灭.

这里返回的是"解码后的文本", 不是固定 8 个字符:

- 每个物理数码管位通常解码成 1 个字符.
- 如果这一位带小数点, 可能会解码成 2 个字符, 例如 `5.`.
- 如果只有小数点点亮, 会解码成 `.`.
- 末尾空白位会被裁掉, 所以返回字符串长度不一定等于 8.

`display_text(window_ms)` 会推进仿真时间, 在给定窗口内持续观察显示内容:

- 从调用点开始先观察这么久.
- 如果这段观察期里整屏显示发生了有效变化, 就把等待顺延到最后一次变化之后又连续稳定了这么久, 再返回该文本.

这样更适合在动态扫描, 切页, 串口更新, 按键切换之后读取"稳定下来的最终显示", 而不是只看某个瞬间.

`display_number()` 和 `display_number(window_ms)` 会从当前显示文本里提取唯一的数值. 如果显示内容里没有数字, 或者同时出现了多个数字, 会直接报错.

- 如果提取到的是纯整数, 返回整数.
- 如果提取到的数字里包含小数点, 返回浮点数.

如果一屏里同时存在多个数字片段, 可以改用 `display_number(start, end)` 或 `display_number(start, end, window_ms)`, 在指定的数码管位范围内提取数字. 位号和 `seg_pattern(1)` 一样, 都是从左到右按 `1..=8` 计数.

带 `window_ms` 的版本会先复用 `display_text(window_ms)` 的整屏稳定等待, 再读取指定范围的数值. 如果题目本身是扫描显示, 但你只关心其中几位, 有时直接用不带窗口的范围版本会更稳.

如果显示里本来就混有空白位, 固定符号位, 或者像温度值 + 等级值这种分段内容, 优先按物理数码管位范围读取, 例如 `display_number(1, 6)` 和 `display_number(8, 8)`. 这样在显示带小数点时, 不会被 `display_text()` 的可变字符长度干扰.
如果你需要刻意卡在某个刷新相位, 闪烁相位, 或者刚发生变化的瞬间, 还是应该先用 `run_to_state(...)` / `run_to_event(...)` 对齐时机, 再读取不带窗口的 `display_text()`.

`display_number(...)` 接受前导零, 但返回值只保留数值本身. 如果题目要求精确判断位宽, 前导零, 空白位, 固定符号位, 请直接对 `display_text(...)` 的结果做字符串切片和正则判断.

这更适合超声波, 温度, 电压这类量测题, 可以直接写布尔表达式判断范围, 避免按整串字符串做数值断言.

`da_value()` 返回当前 DA 输出的原始数值, 范围是 `0..=255`. 对 `PCF8591` 这类 AD/DA 题, 可以直接用它验证按键调节后的 DA 输出是否正确.

`eeprom_byte(addr)` 返回当前 `AT24C02` 指定地址中的原始字节, 范围是 `0..=255`. 对需要验证 EEPROM 块扫描, 指针回绕, 持久化恢复的题目, 可以直接读取指定地址, 不必只靠数码管结果反推内部状态.

`set_eeprom(addr, value)` 会直接覆盖指定地址的 EEPROM 字节. `set_eeprom(addr, [..])` 会从 `addr` 开始连续覆盖多个字节. `set_eeprom([..])` 等价于从 `0x00` 开始写入. 这组接口用于脚本直接布置 EEPROM 初始状态, 不模拟 I2C 总线传输过程.

`uart_config(...)` 是 `uart1_config(...)` 的兼容别名. 当前默认串口格式是 `8` 位数据, `9600` 波特率, `1` 位停止位, `none` 校验位. `uart1_config(...)` 和 `uart2_config(...)` 可分别修改两路串口的外部收发格式, 参数顺序为 `data_bits, baud_rate, stop_bits, parity`.

`stop_bits` 目前支持 `1`, `1.5`, `2`. `parity` 目前支持 `none`, `odd`, `even`, `mark`, `space`, 也接受单字母缩写 `n/o/e/m/s`.

`uart_write()` 是 `uart1_write()` 的兼容别名, 会把文本字节注入 `UART1` 的接收端. `uart2_write()` 则会把文本字节注入 `UART2` 的接收端.

`uart1_write_raw(...)` 和 `uart2_write_raw(...)` 用于注入原始符号数组, 每项范围是 `0..=65535`. 当你需要验证 `9` 位数据, 或者不方便直接当文本处理时, 优先使用 `*_raw` 版本.

`uart_take()` 是 `uart1_take()` 的兼容别名, 返回当前已经从 `UART1` 发出的全部串口文本, 并清空内部发送队列. `uart2_take()` 对 `UART2` 做同样的事情. 如果单片机先发 `OK`, 过一段时间再发 `ERROR`, 只要中间没有先读取, `uart_take()` 默认仍会返回合并后的 `OKERROR`.

`uart_take_raw()`, `uart1_take_raw()`, `uart2_take_raw()` 会返回对应串口当前已经发出的全部原始符号数组, 并清空内部发送队列. 当串口配置为 `9` 位数据, 或者发送内容不适合直接按文本解释时, 请优先使用 `*_take_raw()`.

带 `idle_ms` 参数的版本会按"空闲超时"分段:

- `uart_take(10)` / `uart1_take(10)` / `uart2_take(10)` 只返回当前第 1 段文本, 并只清空这一段.
- `uart_take_raw(10)` / `uart1_take_raw(10)` / `uart2_take_raw(10)` 只返回当前第 1 段原始符号数组, 并只清空这一段.
- 分段规则是: 相邻两次发送之间, 如果 TX 线空闲时间 `>= idle_ms` 毫秒, 就视为新的一段.
- `idle_ms` 必须 `> 0`. 例如 `10` 常用于把 `OK` 和稍后再发出的 `ERROR` 分开读取.

`uart_peek()` / `uart1_peek()` / `uart2_peek()` 用于非破坏读取当前"上位机接收缓冲区". 它们返回当前已经从单片机发出的文本, 但不会清空队列.

`uart_peek_raw()` / `uart1_peek_raw()` / `uart2_peek_raw()` 是对应的原始符号版本, 同样不会清空队列.

带 `idle_ms` 参数的 `uart_peek(10)` / `uart_peek_raw(10)` 也会按同样的空闲超时规则, 只查看当前第 1 段, 但不会消费任何内容.

串口题常见写法:

```rhai
uart_write("00012");
run_ms(220);
assert_eq(uart_take(), "13", "UART1 回显: 返回值错误");
assert_eq(uart_take(), "", "UART1 回显: 缓冲区清空错误");
let text = display_text(30);
assert_eq(text[0..3], "   ", "UART1 回显: 前 3 位空白显示错误");
assert_eq(text[3..8], "00012", "UART1 回显: 后 5 位数值显示错误");
```

如果你需要验证"长时间不读串口, 多条回复是否会合并", 可以这样写:

```rhai
run_ms(1000);
assert_eq(uart_peek(), "OKERROROK", "累计读取: 默认整队列查看错误");
assert_eq(uart_peek(10), "OK", "累计读取: 首段查看错误");
assert_eq(uart_take(10), "OK", "累计读取: 第 1 段读取错误");
assert_eq(uart_take(10), "ERROR", "累计读取: 第 2 段读取错误");
assert_eq(uart_take(10), "OK", "累计读取: 第 3 段读取错误");
```

双串口和 `9` 位符号示例:

```rhai
uart1_config(8, 9600, 1, "none");
uart2_config(9, 19200, 1.5, "even");

uart1_write("OK");
uart2_write_raw([0x141, 0x055]);
run_ms(40);

assert_eq(uart1_take(), "OK", "UART1 示例: 文本输出错误");
let raw = uart2_take_raw();
assert_eq(raw[0], 0x141, "UART2 示例: 第 1 个 9 位符号错误");
assert_eq(raw[1], 0x055, "UART2 示例: 第 2 个 9 位符号错误");
```

如果题目里的 `UART2` 会按 `9` 位数据把收到的符号倒序回发, 可以直接写成:

```rhai
uart2_config(9, 115200, 1, "none");
uart2_write_raw([0x041, 0x155, 0x0AA, 0x1F0]);
run_ms(220);

let raw = uart2_take_raw();
assert_eq(raw[0], 0x1F0, "UART2 倒序回发: 第 1 个符号错误");
assert_eq(raw[1], 0x0AA, "UART2 倒序回发: 第 2 个符号错误");
assert_eq(raw[2], 0x155, "UART2 倒序回发: 第 3 个符号错误");
assert_eq(raw[3], 0x041, "UART2 倒序回发: 第 4 个符号错误");
```

## Rhai 字符串切片和正则

- `regex_is_match(text, pattern)`
- `regex_match(text, pattern)`
- `assert_regex(text, pattern, "label")`

Rhai 自带字符串切片语法, 可以直接写 `text[0..5]`. 这里的范围是 `start..end`, 也就是 0 基, 右边界不包含在结果里.

Rhai 也自带数值解析函数, 例如:

- `parse_int("123")`
- `parse_float("123.45")`
- `parse_int("ff", 16)`

推荐写法:

- 数值部分如果对应固定物理数码管位, 优先用 `display_number(start, end)` 或 `display_number(start, end, window_ms)`.
- 数值部分如果确实要按字符串格式判断, 再用 `display_text(...)[start..end]` 配合 `parse_int(...)` 或 `parse_float(...)`.
- 固定字符, 空白位, 前导零, 分隔符等格式要求, 直接用 `display_text(...)[start..end]` 判断.
- 需要描述整串格式时, 优先用 `assert_regex(...)` 直接给出带标签的失败信息.
- 不要先看当前 `hex` 的输出再反推 `expect`, 应先根据题意, 源码, 手册推导出应有结果, 再写断言.

例如某个 UART 题里, 数码管前 4 位显示 `TI` 计数和小数点, 后 5 位显示最后一次解析出的数值, 可以直接拆开判断:

```rhai
let text = display_text(30);
assert_eq(text[0..4], "015.", "TI 计数页: 前 4 位显示错误");
assert_eq(text[4..9], "00012", "TI 计数页: 后 5 位数值显示错误");
```

如果前 3 位是会随中断重入次数变化的计数, 更推荐把固定格式和数值范围拆开写, 不要把整串计数写死:

```rhai
let text = display_text(30);
assert_eq(text[3..4], ".", "TI 计数页: 小数点显示错误");
assert_in(parse_int(text[0..3]), 2..=999, "TI 计数页: 计数值超出范围");
assert_eq(text[4..9], "00000", "TI 计数页: 后 5 位数值显示错误");
```

## 数值容差设计

如果是根据已有的题目来编写评测脚本, 数值类断言的窗口不应拍脑袋决定. 更稳妥的做法是先从题面约束出发, 再把误差沿着计算链路传播到最终显示值.

推荐步骤如下:

- 先找题面或手册里已经给出的误差来源, 例如传感器精度, 显示位数, 采样分辨率, 定时周期, 通信波特率误差.
- 把脚本注入的条件视为"真实输入", 先推导出被测程序允许读到的原始量范围.
- 再根据题目的计算公式, 把原始量范围传播到派生量, 例如液位, 体积, 剩余百分比, 重量, 频率变化量.
- 最后再根据显示规则收口, 例如"保留 1 位小数", "整数显示", "单位换算后再显示".

一个常见误区是直接看当前 `hex` 的输出, 然后围着这个输出随手留 `+-0.1` 或 `+-1` 的窗口. 这样做无法区分"题面本来允许的波动"和"实现里真实存在的计算错误".

更推荐把窗口写成下面这种可追溯形式:

- 输入容差窗口. 由题面精度直接给出.
- 派生量窗口. 由公式推导得到.
- 显示窗口. 由显示位数和取整规则得到.
- 脚本断言窗口. 应覆盖上面三层推导后的结果, 而不是覆盖某个 `hex` 的偶然输出.

以 `na16` 的圆柱体体积页为例:

- 题面给出超声波液位测量精度为 `+-2cm`.
- 若脚本注入距离为 `87cm`, 那么允许测到的距离区间可写成 `[85, 89] cm`.
- 圆柱体参数为 `H = 2.5m`, `r = 2.0m`, 则液位高度区间为 `[1.61, 1.65] m`.
- 由 `V = 3.14 * r^2 * h` 可得体积区间为 `[20.2216, 20.7240] m^3`.
- 若题面要求体积显示保留 `1` 位小数, 则脚本窗口至少应覆盖 `20.2..20.7`.
- 剩余空间百分比可由 `p = distance / H * 100` 得到区间 `[34.0, 35.6] %`.
- 若题面只说"整数显示", 但没有说明是截断, 四舍五入, 还是向上取整, 那么脚本窗口应至少覆盖 `34..=36`. 如果你想强制要求某一种取整语义, 应在题目说明或评测说明里把这条规则写死.

还有一个很重要的检查点是"同源量一致性". 如果同一页上的两个字段都来自同一次距离测量, 那么它们反推出的距离区间应当彼此重合. 例如:

- 体积显示 `20.3` 对应的液位高度约为 `1.62m`, 反推距离约为 `88cm`.
- 剩余空间显示 `36` 若按直接计算剩余百分比再取整理解, 更接近 `90cm`.
- 这两个字段如果长期同时出现, 往往不是"容差边缘", 而是两个字段用了不同的计算或取整路径.

对时间戳类断言, 也不要直接围着某个现象值留 `+-1s`. 更稳妥的做法是先把题面里的采样节拍和脚本场景时序展开:

- 先确认题面是否写了"每隔 1s 采样", "上电后第 1 秒开始", "持续 5s" 这类节拍约束.
- 再把脚本里每次 `run_ms(...)`, `display_text(...)`, `watch_led_stats(...)` 等等函数会推进多少仿真时间算清楚.
- 然后把输入翻转时刻和采样时刻对齐, 推导最早可能命中的采样点和最晚可能命中的采样点.
- 如果题面没有写死门宽, 锁存时刻, 或"在本拍上报"还是"下一拍确认后上报", 断言窗口应覆盖这些合法实现分支.

例如 `na16` 的 `503` 趋势异常场景里, 频率现在就是每 `1000ms` 恰好切一次, 也就是把
`1500 -> 1300 -> 1000 -> 800 -> 500` 这些翻转点直接压在 `1s` 采样边界上. 这样重新推导断言时,
要覆盖的就不再是"稳定观察余量", 而是边界先后次序本身带来的合法分支:

- 如果实现按当下时刻读取频率, 或在边界上先完成本拍采样再切到下一档, 第 `5` 个连续下降样本会落在 `23:33:49.xx`.
- 如果实现按完整 `1s` 门宽统计, 或在下一拍才确认完整 `5s` 趋势窗口, 则会落在 `23:33:50.xx`.
- 因此脚本应接受 `23:33:49` 和 `23:33:50` 这两个秒值. 这个窗口仍然是由题面采样规则和场景相位共同推导出来的, 不是围着某个 `hex` 的偶然输出取值.

因此, 容差窗口的设计应优先回答两个问题:

- 这个范围能否从题面精度和公式严格推导出来.
- 同一组显示字段是否能由同一组底层测量值同时推出.

如果这两个问题都答不上来, 更应该先回到题面和公式重新推导, 而不是继续放宽脚本窗口.

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
- 对 "每 0.1s / 0.2s 切换一次亮灭状态" 这类固定节奏闪烁, 优先断言 `stats.change_frequency_hz`.
- `stats.change_frequency_hz` 现在按"相邻状态切换间隔是否稳定"来估算. 只有中间那段切换间隔足够接近时它才有效.
- 观察窗口前后的稳定亮 / 稳定灭不会被算作频率突变. 也就是说, 先静止再规律闪烁, 或规律闪烁后再停住, 仍然可以得到有效频率.
- 如果整个观察窗口里 LED 一直没变化, `stats.change_frequency_hz` 会返回 `0`.
- 如果中间各次切换间隔差异过大, `stats.change_frequency_hz` 会是 `NaN`.
- 如果题面写的是完整闪烁周期频率, 需要先换算到状态切换频率再断言.
- `stats.changes` 更适合判断 "完全不闪烁" 这类场景, 例如 `assert_eq(stats.changes, 0, "...")`.
- `stats.pwm_frequency_hz` 和 `stats.duty_percent` 只用于 PWM, 不要拿来判断普通闪烁.

例如 `led_flicker` 可以直接这样写:

```rhai
run_ms(20);
let stats = watch_led_stats(L1, 1000);
assert_in(stats.change_frequency_hz, 9..=11, "L1 闪烁统计: 状态切换频率超出范围");
```

- 但是评测最好留有余量, 防止误差.

例如 `led_pwm` 可以这样写:

```rhai
run_ms(220);
let stats = watch_led_stats(L1, 40);
assert_in(stats.pwm_frequency_hz, 950..=1050, "L1 PWM: 频率超出范围");

assert_in(stats.duty_percent, 8..=12, "L1 PWM: 占空比超出范围");
```

## 数码管段码

- `seg_raw(1)`
- `seg_pattern(1)`
- `set_seg_decode(0x3F, "0")`
- `set_seg_blank(0x00)`

说明:

- `seg_raw(index)` 返回锁存到该位数码管上的原始字节.
- `seg_pattern(index)` 返回按 `!raw` 归一化后的段码模式, 更适合直接按 `0x3F` 这类常见段码表判断.
- `run_to_state("seg.d3.visible", true, ...)` 可以等待某一位重新显示为非空白, 适合处理闪烁字段.
- `set_seg_decode(pattern, text)` 用于自定义 `display_text()` 的解码规则.
- `set_seg_blank(pattern)` 将某个模式视为留空.

默认已经内置了 `0-9 - P E L F H C` 的解码映射.

## 断言和调试

- `assert(cond, "message")`
- `assert_eq(actual, expected, "label")`
- `assert_regex(text, pattern, "label")`
- `assert_in(actual, 10..=12, "label")`
- `ckpt(index, condition, expected, || { ... })`
- `print(anything)`

`assert_eq(...)` 要求 `actual` 和 `expected` 是同类型. 适合字符串, 整数, 浮点, 布尔等直接相等比较. 失败时会同时打印 `expected` 和 `actual`.
`assert_regex(...)` 用于判断左侧字符串是否匹配右侧正则. 失败时会同时打印正则和实际字符串, 也会保留 `label`.
`assert_in(...)` 适合整数和浮点数的区间判断. 目前使用 Rhai 的整数 range 语法, 支持 `a..b` 和 `a..=b`. 对浮点实际值会按对应的整数边界比较. 失败时会同时打印期望区间和实际值.

断言文案建议遵循下面几条:

- `label` 和失败说明之间使用非空白分隔符, 例如 `: `, 不要只靠空格直接拼接.
- 不要只写对象名, 应明确说明失败语义, 例如 `时间显示错误`, `PWM 占空比超出范围`, `串口回复错误`.
- 优先使用 `assert_eq(...)`, `assert_in(...)`, `assert_regex(...)` 这类会自动输出 `expected` 和 `actual` 的断言.
- 如果必须使用裸 `assert(...)`, 需要在消息里手动补上关键现场, 例如 `actual=`, `expected=`, `display=` 或 `reply=`.

`ckpt(...)` 用于定义一个可以继续执行的评测点. 它会执行闭包里的脚本逻辑:

- 如果闭包内所有断言都通过, 记录一条 `通过` 结果.
- 如果闭包内任意 `assert_*`, `run_to*` 超时, 或其他脚本运行时错误失败, 记录一条 `失败` 结果, 但脚本继续往后执行.
- 脚本结束后, 评测器会自动输出终端 Markdown 表格, 列出每个 checkpoint 的序号, 测试条件, 期望结果, 实际结果和状态.
- 失败时, 终端表格里的 `实际结果` 会显示诊断信息. 详细错误链请看同一个 checkpoint 的 `info` tracing.
- 如果脚本里至少定义了一个 `ckpt(...)`, 且最终存在失败项, 进程仍会以失败退出, 方便判题和 CI 感知失败.
- 在默认 `info` 级别下, 每个 `ckpt(...)` 执行结束后, 评测器都会输出一条 tracing 日志, 包含序号, 状态, 测试条件, 期望结果, 实际结果和当时的 `sim_time_ns`.

推荐把 `ckpt(...)` 理解成"显式评测点"而不是"弱化版 assert". 普通 `assert_*` 仍然保持"立即失败"语义, 更适合公共辅助函数和那些一旦失败就不适合继续推进场景的前置条件.

需要特别注意一条执行语义:

- `ckpt(...)` 只会在当前 checkpoint 失败后, 继续执行后面的下一个 `ckpt(...)`.
- 它不会让当前闭包在 `assert_*` 失败后继续往下执行.
- 也就是说, 一旦闭包里的某条 `assert_*` 失败, 这个闭包后面的语句就不会再运行.

因此, 在 `ckpt(...)` 闭包里应尽量遵循"先操作, 后断言"的写法:

- 先完成按键, 串口发送, 传感器设置, 页面切换, 采样读取等动作.
- 再对前面采集到的结果统一做 `assert_*`.
- 如果一个 checkpoint 里还要继续做后续动作, 不要在这些动作之前插入可能失败的 `assert_*`.

尤其是这类多阶段流程:

- 先发第一条串口命令, 再发第二条串口命令, 最后统一检查两次回复.
- 先切完多个页面并把每页文本都读出来, 最后再统一检查每一页.
- 先设置一个合法状态, 再发送非法输入验证保持行为, 最后再一起检查回复和显示.

`ckpt(...)` 闭包返回值会作为该评测点的"实际结果"写入表格:

- 如果闭包没有显式返回值, 表格中会写 `符合期望`.
- 如果闭包返回字符串, 数字等值, 表格中会直接显示该值.
- 如果闭包失败, 表格中会显示失败诊断信息, 详细错误链保留在 `info` tracing 中.

一个常见写法如下:

```rhai
ckpt(1, "上电默认时间页", "显示 23-59-50", || {
    let text = display_text(30);
    assert_eq(text, "23-59-50", "默认时间页: 显示内容错误");
    text
});

ckpt(2, "非法时间配置", "返回 ERROR 且显示保持不变", || {
    uart_write("(T:286344)");
    run_ms(160);
    assert_eq(uart_take(), "ERROR", "非法时间配置: 串口回复错误");
    let text = display_text(30);
    assert_eq(text, "23-33-44", "非法时间配置: 显示保持错误");
    text
});
```

如果一个 checkpoint 需要多个阶段, 推荐写成下面这样:

```rhai
ckpt(3, "两次串口设置都应成功", "OK -> OK", || {
    send_uart("(H1,1.8)");
    let first = uart_take();

    send_uart("(H2,0.5)");
    let second = uart_take();

    assert_regex(first, "^OK", "第一次设置: 串口回复错误");
    assert_regex(second, "^OK", "第二次设置: 串口回复错误");
    first + "," + second
});
```

不推荐写成下面这样:

```rhai
ckpt(4, "反例", "不要这样写", || {
    send_uart("(H1,1.8)");
    let first = uart_take();
    assert_regex(first, "^OK", "第一次设置: 串口回复错误");

    send_uart("(H2,0.5)");
    let second = uart_take();
    assert_regex(second, "^OK", "第二次设置: 串口回复错误");
    first + "," + second
});
```

上面的反例里, 如果第一条 `assert_regex(...)` 失败, 第二次 `send_uart(...)` 根本不会执行, 后续流程也就被吞掉了.

调试建议:

- 想在脚本中途打印, 用 `print(...)`.
- 想看固定时刻全量状态, 用 `dump` 子命令.
- 想看寄存器, 锁存器, LED, 段码, UART 等综合快照, 用 `print(snapshot_text())`.
- 如果脚本较长, 可以直接看每个 `ckpt(...)` 结束后的 `info` tracing 摘要, 快速判断当前跑到哪里, 哪个 checkpoint 刚通过或失败.

## 示例

```rhai
run_ms(220);
let text = display_text(30);
assert_eq(text[0..7], "       ", "上电检查: 前 7 位空白显示错误");
assert_eq(parse_int(text[7..8]), 0, "上电检查: 末位数值错误");

set_key(S4, true);
run_ms(220);
let value = parse_int(display_text()[7..8]);
assert_eq(value, 1, "S4 按下: 末位数值错误");
assert(led_on(L1), "S4 按下: L1 状态错误, 期望点亮, 实际未点亮");

set_temperature_c(25.9375);
run_ms(700);
assert_eq(display_number(1, 6), 25.500, "9bit 采样: 温度显示错误");
assert_eq(display_number(8, 8), 0, "9bit 采样: 精度等级错误");

print(snapshot_text());
```
