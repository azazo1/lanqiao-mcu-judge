# C51 CLI 编译

本文说明如何在命令行下把 sample 工程编译成 `hex`.

前提:

- 可以直接调用 `C:/Keil_v1/UV4/UV4.exe`.
- 工程里的器件头文件和工具链配置已经在 `uvproj` 中设置完成.

## 批量构建

推荐直接使用 `UV4` 的 batch build.

命令结构可以写成:

```powershell
C:/Keil_v1/UV4/UV4.exe -b .\sample\arith_bench\prj\arith_bench.uvproj -j0 -t arith_bench -o .\sample\arith_bench\prj\Objects\uv4.log
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
