# C51 CLI 编译

本文说明如何在命令行下把 sample 工程编译成 `hex`.

## Windows

- 可以直接调用 `C:/Keil_v5/UV4/UV4.exe`.
- 工程里的器件头文件和工具链配置已经在 `uvproj` 中设置完成.

## 批量构建

推荐直接使用 `UV4` 的 batch build.

命令结构可以写成:

```powershell
C:/Keil_v5/UV4/UV4.exe -b .\sample\arith_bench\prj\arith_bench.uvproj -j0 -t arith_bench -o .\sample\arith_bench\prj\Objects\uv4.log
```

这几个参数的作用如下:

- `-b`: 按工程文件执行批量构建.
- `-j0`: 关闭交互界面阻塞, 适合命令行调用.
- `-t arith_bench`: 指定要构建的 target 名称.
- `-o ...`: 把构建日志写到指定文件.

## 构建产物

构建成功后, `hex` 默认会输出到:

```text
sample\arith_bench\prj\Objects\arith_bench.hex
```

日志文件也会输出到:

```text
sample\arith_bench\prj\Objects\uv4.log
```

如果要编译别的 sample, 一般只需要同时替换这几项:

- `uvproj` 路径
- `-t` 后面的 target 名称
- `-o` 后面的日志文件名

## 查看结果

可以直接检查 `hex` 是否存在:

```powershell
Get-ChildItem .\sample\arith_bench\prj\Objects\arith_bench.hex
```

也可以查看日志, 确认是否为 `0 Error(s), 0 Warning(s)`:

```powershell
Get-Content .\sample\arith_bench\prj\Objects\uv4.log
```

## 后续仿真

拿到 `hex` 之后, 可以直接运行评测:

```powershell
stcjudge.exe run --hex .\sample\arith_bench\prj\Objects\arith_bench.hex --script .\sample\arith_bench\judge\smoke.rhai
```

## macOS

Keil 官方工具只支持 Windows 环境. 在 macOS 上, 推荐使用 CrossOver 承载 Keil C51 和 uVision, 再复用仓库里现成的 `uvproj` 工程.

### 环境准备

推荐顺序如下:

1. 在 macOS 上安装 CrossOver.
2. 创建一个 bottle, 例如 `c51`.
3. 在这个 bottle 里安装 Keil C51 和 uVision.
4. 找到 CrossOver bottle 里的 `C:\Keil_v5`.
5. 从已经配置好 STC 器件库的 Windows Keil 环境复制 STC 相关文件到这个 `C:\Keil_v5`.
6. 启动 Keil, 打开任意 sample 的 `prj/*.uvproj`, 确认器件显示为 `STC15F2K60S2 Series`, 头文件为 `STC15F2K60S2.H`.

需要复制的内容如下:

1. `C:\Keil_v5\UV4\STC.CDB`
2. `C:\Keil_v5\C51\INC\STC\`
3. 在 `C:\Keil_v5\TOOLS.INI` 的 `[UV2]` 章节加入:

```ini
CDB0=UV4\STC.CDB ("STC MCU Database")
```

如果你的 CrossOver bottle 名字是 `c51`, 对应的 macOS 路径通常类似:

```text
~/Library/Application Support/CrossOver/Bottles/c51/drive_c/Keil_v5
```

这个仓库的 sample 工程已经依赖了 STC 的器件定义. 例如 `sample/arith_bench/prj/arith_bench.uvproj` 里会引用:

- `Vendor = STC`
- `Device = STC15F2K60S2 Series`
- `RegisterFile = STC15F2K60S2.H`

如果没有把上面的 STC 器件库复制进 CrossOver 里的 Keil, uVision 往往无法按原工程直接构建.

这条链路里, 有一部分来自官方文档, 也有一部分是当前仓库和 CrossOver 环境下的实测做法:

- Keil 官方文档明确支持 `UV4 -b <project> -t <target> -o <log>` 这样的批量构建方式: <https://www.keil.com/support/man/docs/uv4/uv4_commandline.asp>
- CodeWeavers 官方文档明确给出了 macOS 下从终端运行 Windows 程序的方法, 包括 `wine --cx-app executable.exe` 这样的调用方式: <https://support.codeweavers.com/en_US/run-a-windows-app-from-terminal>
- STC 官方下载页明确提供了 Keil 的 `UV2.CDB` 和 `UV3.CDB` 插件下载入口: <https://www.stcmicro.com/cn/rjxz.html>
- 当前仓库在 macOS 下的实测可用方法, 是从已经配置好的 Windows Keil 中复制 `STC.CDB`, `C51\INC\STC\`, 并在 `TOOLS.INI` 里加入 `CDB0=...`, 再通过 CrossOver bottle 里的 Keil 复用原工程.

### 环境文件

`just` 会自动加载项目根目录 `.env`.

推荐先复制一份示例:

```bash
cp .env.example .env
```

然后把 `.env` 改成你自己的路径. 对于 CrossOver, 最少需要这几项:

```bash
KEIL_WINE="~/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"
KEIL_CROSSOVER_BOTTLE="c51"
KEIL_DRIVE_C="~/Library/Application Support/CrossOver/Bottles/c51/drive_c"
KEIL_UV4='C:\Keil_v5\UV4\UV4.exe'
```

脚本会自动展开 `.env` 里的 `~/...`, 所以这里不必写完整的 `/Users/<用户名>/...`.

如果你不想用默认的 `.env`, 也可以在运行前指定:

```bash
just --dotenv-path /path/to/keil.env build-sample arith_bench
```

### 仓库内命令入口

macOS 侧已经提供:

```bash
just build-sample arith_bench
```

它会调用 `bash scripts/build-sample-macos.sh arith_bench`, 自动完成:

- 查找 `sample/<name>/prj/*.uvproj`
- 解析 target 名称
- 调用 `UV4.exe -b ... -j0 -t ... -o ...`
- 检查 `sample/<name>/prj/Objects/*.hex`

### 自检脚本

如果你想先确认兼容层里的 Keil 是否已经具备 STC15 支持, 可以执行:

```bash
bash scripts/keil-env-doctor.sh arith_bench
```

或者:

```bash
just keil-doctor
```

它会检查这些关键项:

- `wine` 或 `KEIL_UV4_LAUNCHER`
- `drive_c` 和 `Keil_v5`
- `UV4/UV4.exe`
- `TOOLS.INI`
- `UV4/STC.CDB`
- `C51/INC/STC/STC15F2K60S2.H`

### CrossOver 环境示例

`.env.example` 给的是最常见的 CrossOver 配置. 对应到实际机器时, 往往只需要改这三处:

- `KEIL_WINE`
- `KEIL_CROSSOVER_BOTTLE`
- `KEIL_DRIVE_C`

### 构建产物

成功后会在 sample 自己的工程目录下看到:

```text
sample/arith_bench/prj/Objects/arith_bench.hex
sample/arith_bench/prj/Objects/uv4.log
```

### 常见问题

- `未找到可执行文件`: 说明 `KEIL_UV4_LAUNCHER` 或 `KEIL_WINE` 路径不对.
- `未能自动定位 drive_c`: 先设置 `KEIL_CROSSOVER_BOTTLE`, `KEIL_WINEPREFIX` 或 `KEIL_DRIVE_C`.
- `.env` 改了但命令没生效: 检查变量名是否和 `.env.example` 一致, 路径里有空格时保留引号.
- `未找到 hex`: 先检查 `sample/<name>/prj/Objects/uv4.log`.
- 器件或头文件报错: 重新检查 `STC.CDB`, `C51\INC\STC\`, 以及 `TOOLS.INI` 的 `[UV2]` 章节是否都已从 Windows Keil 正确复制过来.
- 器件库仍未生效: 检查 `Keil_v5/UV4/STC.CDB`, `Keil_v5/C51/INC/STC/STC15F2K60S2.H`, 以及 `TOOLS.INI` 是否包含 `CDB0=UV4\STC.CDB ("STC MCU Database")`.
- 工程能打开但不能批量构建: 在 uVision 里手动打开一次工程, 确认 target 名称和器件库都已生效.
