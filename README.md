# 蓝桥杯单片机评测

类似 4t 的功能, 能够本地运行 C51 单片机 hex 文件评测.

支持编写评测脚本.

仅针对考纲内容级别的硬件进行模拟仿真, 不对更深层的时序和进行仿真.

## 使用

可执行文件名为 `stcjudge`. 例如执行 `cargo build --release` 之后, 可以直接运行 `target/release/stcjudge`.

> [!note]
> 使用 `--release` 运行会比开发中运行快上不少.

按脚本文件执行:

```bash
stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --script sample/key_seg/judge/smoke.rhai
```

从标准输入执行 Rhai:

```bash
stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --stdin < sample/key_seg/judge/smoke.rhai
```

交互式执行:

```bash
stcjudge repl --hex sample/key_seg/prj/Objects/key_seg.hex
```

查看 Rhai 脚本逐语句 tracing:

```bash
RUST_LOG=debug stcjudge run --hex sample/key_seg/prj/Objects/key_seg.hex --script sample/key_seg/judge/smoke.rhai
```

固定时刻抓取快照:

```bash
stcjudge dump --hex sample/key_seg/prj/Objects/key_seg.hex --ms 220
```

## 评测脚本

- 评测脚本约定放在 `sample/xxx/judge/`.
- 详细手册见 [docs/judge-script-manual.md](docs/judge-script-manual.md).
- 现在支持 `print(...)`, `watch_led_stats(...)`, `display_text(window_ms)`, `display_number(...)`, `key_mode(...)`, `jumper_on(...)`, `jumper_off(...)`, `jumper_installed(...)` 以及内置常量 `L1..L8`, `S4..S19`, `RB2/RB3/RB4/RD1`, `KEYBOARD/KBD`, `BUTTON/BTN`, `SIG_OUT/NET_SIG`.
- `RUST_LOG=debug` 时会输出 Rhai 脚本逐语句执行进度, 包括步号, 行列号, 调用层级和当前源码行.
- 默认跳帽状态按原理图建模, `NET_SIG` 不会自动连到 `SIG_OUT`. 如果题目需要把 NE555 输出送到 `P3.4/T0`, 需要在脚本里显式写 `jumper_on(NET_SIG, SIG_OUT)`.

## 仿真时钟

- 当前 CPU 基准按 STC15F2K60S2 的 1T 模式和 12MHz 主时钟建模.
- `run_ms` 和 `run_us` 只推进虚拟时间, 不等待真实时间, 所以评测速度不受 wall clock 限制.
- 和真机相比, 如果题目依赖外部晶振配置、时钟分频、模拟器件建立时间或未实现的片上外设细节, 结果仍可能有偏差.

## 模块

- `ds1302`, `pcf8591`, `at24c02`, `ne555`, `seg`, `超声波`, `按键` 已经拆成独立 Rust 模块.
- 当前可以直接在脚本中读取 `relay_on()`, `buzzer_on()`, `motor_on()` 三类板载输出状态.

## docs

- SCH_V31.pdf: 开发板原理图.
- STC15_DS.pdf: STC15 系列芯片的用户手册.
- knowledge-points.pdf: 十五届单片机考纲.
- Datasheet: 板上各个外设的用户手册.

## sample

评测真题示例.
