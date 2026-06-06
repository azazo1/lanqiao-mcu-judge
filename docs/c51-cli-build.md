# C51 CLI 编译

本文说明如何在命令行下把 sample 工程编译成 `hex`.

前提:

- `UV4.exe` 已经可以直接从命令行调用, 或者其所在目录已经加入 `PATH`.
- 工程里的器件头文件和工具链配置已经在 `uvproj` 中设置完成.

## 批量构建

最稳妥的方式是直接让 `UV4` 按工程文件批量构建:

```powershell
UV4.exe -b "sample\float_bench\prj\float_bench.uvproj" -o "sample\float_bench\prj\build.log"
```

构建完成后, `hex` 默认会输出到:

```text
sample\float_bench\prj\Objects\float_bench.hex
```

如果要编译别的 sample, 只需要替换 `uvproj` 路径即可.

## 查看构建结果

可以直接检查 `hex` 是否存在:

```powershell
Get-ChildItem "sample\float_bench\prj\Objects\float_bench.hex"
```

也可以打开批量构建日志确认是否为 `0 Error(s), 0 Warning(s)`:

```powershell
Get-Content "sample\float_bench\prj\build.log"
```

## 后续仿真

拿到 `hex` 之后, 可以直接运行评测:

```powershell
.\target\release\stcjudge.exe run --hex "sample\float_bench\prj\Objects\float_bench.hex" --script "sample\float_bench\judge\smoke.rhai"
```
