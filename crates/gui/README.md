# stcjudge-gui

`stcjudge-gui` 是 `stcjudge` 的原生桌面调试台, 基于 `egui` 和 `eframe` 实现, 直接调用 workspace 内的核心仿真库.

## 功能

- 调试台: 加载 HEX, 复位, 运行, 单步, 查看数码管, LED, relay, buzzer, motor, 端口和锁存器.
- 输入注入: 支持按键矩阵, 温度, 距离, NE555 频率, RD1/RB2 电压, 跳帽和 UART 输入.
- 评测运行: 选择 HEX 和 Rhai 脚本, 显示进度条, 实时更新 ckpt 表格.
- 脚本工作台: 编辑 Rhai 脚本, 插入常用 API 片段, 直接运行当前脚本.
- 波形: 配置 HTML, JSON, msgpack 导出路径和时间窗口. 波形查看界面是独立 HTML, 导出后在浏览器中打开.
- 软件更新: 启动后静默检查, 状态栏版本号可打开更新窗口; 下载完成只进入等待重启状态, 由用户点击 "重启并更新" 才替换.

## 运行

```bash
just gui
```

也可以直接运行:

```bash
cargo run --release -p stcjudge-gui
```

## 调试实例

```bash
just debug
```

调试实例的数据目录为 `target/gui-debug/`, 日志写入 `target/gui-debug/app.log`, 日志级别开到 trace; 因为它使用独立的数据目录, 单实例锁也与正式实例互不影响, 可以并行运行.

## 数据目录与日志

- 数据目录默认是平台的应用数据目录下的 `stcjudge-gui`, 可以用 `STCJUDGE_GUI_DATA_DIR` 覆盖.
- 设置 (自动检查更新, 跳过的版本) 存在数据目录的 `settings.json`, 带结构版本号, 升级时按版本迁移.
- 主日志文件默认在数据目录的 `logs/app.log`, 可以用 `STCJUDGE_GUI_LOG_FILE` 覆盖; 日志按天与单文件大小轮转, 默认留存 10 个归档文件.
- 更新包与替换脚本存放在数据目录的 `update/` 下, `apply-update.log` 保留 macOS 替换过程的完整输出.

## 版本号

- 发布构建显示 `v1.2.3`, 非 tag 提交显示 `v1.2.3-a1b2c3d`, 工作区有未提交改动时显示 `v1.2.3^a1b2c3d`, 均由 `PROJECT_BUILD_VERSION` 在构建期注入.
- 直接 `cargo run -p stcjudge-gui` 的开发构建显示 `dev-build`, 不参与更新检查.

## 自动更新

- 只处理正式 release 资产 `stcjudge-gui-<version>-<platform>-<arch>.<ext>`, 与 [docs/release.md](../../docs/release.md) 的命名约定严格对应.
- 下载带 SHA256 校验, 支持断点续传; 下载完成停在 "等待重启", 不会自动退出或自动替换.
- linux 与 windows 替换运行中的二进制并拉起新进程; macOS 交接给脱离进程的替换脚本, 便携运行 (直接跑二进制) 则打开镜像引导手动安装.
