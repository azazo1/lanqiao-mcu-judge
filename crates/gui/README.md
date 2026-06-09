# stcjudge-gui

`stcjudge-gui` 是 `stcjudge` 的原生桌面调试台, 基于 `egui` 和 `eframe` 实现, 直接调用 workspace 内的核心仿真库.

## 功能

- 调试台: 加载 HEX, 复位, 运行, 单步, 查看数码管, LED, relay, buzzer, motor, 端口和锁存器.
- 输入注入: 支持按键矩阵, 温度, 距离, NE555 频率, RD1/RB2 电压, 跳帽和 UART 输入.
- 评测运行: 选择 HEX 和 Rhai 脚本, 显示进度条, 实时更新 ckpt 表格.
- 脚本工作台: 编辑 Rhai 脚本, 插入常用 API 片段, 直接运行当前脚本.
- 波形: 配置 HTML, JSON, msgpack 导出路径和时间窗口. 波形查看界面是独立 HTML, 导出后在浏览器中打开.

## 运行

```bash
just gui
```

也可以直接运行:

```bash
cargo run --release -p stcjudge-gui
```
