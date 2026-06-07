# C51 CLI 编译

本文说明如何在命令行下把 sample 工程编译成 `hex`.

前提:

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

## SDCC + packihx 补充流程

这一节作为上面 `UV4` 流程的补充, 偏向 Unix 等无法使用 Keil 的用户.

前提:

- `sdcc` 和 `packihx` 已经在 `PATH` 中.
- sample 目录保持当前约定, 也就是源码在 `sample/<name>/src`, 产物在 `sample/<name>/prj/Objects`.

推荐直接使用项目里的 recipe:

```bash
just build-sample-sdcc key_seg
```

这条 recipe 会做几件事:

- 先调用仓库里的 Rust 独立 bin `build-sample-sdcc`.
- 读取 `sample/<name>/src` 下的 `.c` 和 `.h`.
- 在临时目录里对常见的 `Keil C51` 语法做一次自动兼容转换.
- 如果 `sample/<name>/prj` 下存在 `uvproj`, 会读取其中的 `<Cpu>` 配置, 推导 `IRAM`, `XRAM`, `IROM` 大小, 尽量贴近原工程的芯片内存布局; 缺失时默认使用 `stc15f2k60s2` 对应的 `iram=256`, `xram=1792`, `code=61433`.
- 先把每个 `.c` 编译成 `.rel`.
- 再按 `small -> small + stack-auto -> large + stack-auto` 的顺序尝试链接成 `.ihx`.
- 最后调用 `packihx` 生成 `hex`.

当前自动处理的差异主要包括:

- 把 `#include <STC15F2K60S2.H>` 映射到 `sdcc` 自带的 `stc12.h`, 并补上 `T2H`, `T2L`, `P30..P37`, `P40..P47`, `P50..P53` 这类本项目 sample 已经用到的定义.
- 把 `#include "intrins.h"` 里的 `_nop_()` 映射为 `sdcc` 可接受的内联汇编.
- 把 `idata`, `pdata`, `xdata`, `bdata`, `code`, `bit` 这些关键字转换为 `sdcc` 写法.
- 把 `unsigned char data i, j;` 这一类 `Keil` 局部 `data` 存储类声明降级成普通局部变量, 以兼容 `stack-auto` 和 `reentrant` 场景.
- 把 `interrupt n`, `using n`, `sbit x = Pn^m;` 转成 `sdcc` 语法.
- 把 `iic.c` 中的 `static void I2C_Delay(...)` 自动放开为可跨文件调用, 并在缺失声明的 `iic.h` 里补上原型.
- 把 `extern char putchar(char ch)` 这一类 `Keil` 常见写法转换为 `sdcc stdio.h` 兼容的 `int putchar(int ch)`.
- 内置一个最简 `sscanf` 兼容层, 当前支持 `%u`, `%bu`, `%lu`, `%d`, `%bd`, `%ld`, `%f`, `%n`, 以及前置宽度写法如 `%02d`, `%02u`, `%02bu`; 另外会宽松接受 `%.2f` 这种格式串并忽略其中的小数位说明, 用于兼容现有工程里的非标准写法.

以 `key_seg` 为例, 构建完成后会产出:

```text
sample/key_seg/prj/Objects/key_seg.ihx
sample/key_seg/prj/Objects/key_seg.hex
```

如果要编译别的 sample, 只需要替换参数:

```bash
just build-sample-sdcc ds1302
just build-sample-sdcc uart
```

如果只想手动调用底层构建器, 也可以直接运行:

```bash
cargo run --quiet --bin build-sample-sdcc -- key_seg
```

当前仓库里的 sample 已经逐个验证可以通过这条流程生成 `hex`.

需要注意的是, 这套兼容逻辑目前只覆盖了仓库 sample 中已经实际出现的常见差异, 目标是让 `sdcc + packihx` 能顺手生成 sample `hex`, 不是完整的 `Keil C51` 兼容层. 如果后续 sample 引入了新的专有语法, 需要继续在这个 Rust bin 中补转换规则.
