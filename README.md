# 蓝桥杯单片机评测

类似 4t 的功能, 能够本地运行 C51 单片机 hex 文件评测.

支持编写评测脚本.

仅针对考纲内容级别的硬件进行模拟仿真, 不对更深层的时序和进行仿真.

## 使用

可执行文件名为 `stcjudge`. 例如执行 `cargo build --release` 之后, 可以直接运行 `target/release/stcjudge`.

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

固定时刻抓取快照:

```bash
stcjudge dump --hex sample/key_seg/prj/Objects/key_seg.hex --ms 220
```

## 评测脚本

- 评测脚本约定放在 `sample/xxx/judge/`.
- 详细手册见 [docs/rhai-script-manual.md](/Users/azazo1/pjs/rust/lanqiao-mcu-judge/docs/rhai-script-manual.md).
- 现在支持 `print(...)`, `watch_led_changes(...)`, `display_text(window_ms)` 以及内置常量 `L1..L8`, `S4..S19`, `RB2/RB3/RB4/RD1`.

## 仿真时钟

- 当前 CPU 基准按 STC15F2K60S2 的 1T 模式和 35MHz 主时钟建模.
- `run_ms` 和 `run_us` 只推进虚拟时间, 不等待真实时间, 所以评测速度不受 wall clock 限制.
- 和真机相比, 如果题目依赖外部晶振配置、时钟分频、模拟器件建立时间或未实现的片上外设细节, 结果仍可能有偏差.

## docs

- SCH_V31.pdf: 开发板原理图.
- STC15_DS.pdf: STC15 系列芯片的用户手册.
- knowledge-points.pdf: 十五届单片机考纲.
- Datasheet: 板上各个外设的用户手册.

## sample

评测真题示例.
