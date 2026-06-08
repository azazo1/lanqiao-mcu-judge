# Keil 编译产物调试与 `peek_*` / `poke_*`

这份文档只讨论当前项目真正使用的 Keil C51 和 BL51 原生产物.

先看结论:

- 先跑 `just analyze-objects <sample> [pattern]`.
- 地址定位主要看 `samples/<name>/prj/Listings/<target>.m51`.
- 源码声明和局部变量分配主要看 `samples/<name>/prj/Listings/*.lst`.
- `samples/<name>/prj/Objects/*.hex` 主要用于运行仿真, 不是定位变量地址的主入口.

## 为什么 `ad_da` 没有那些文件

`ad_da` 没有你前面提到的 `map`, `mem`, `sym`, `rst` 这些文件, 不是构建不完整, 而是因为它走的是 Keil C51 和 BL51 的原生输出链路.

这个链路里, 真正有用的信息主要在:

- `Listings/ad_da.m51`
- `Listings/main.lst`
- 其他模块自己的 `Listings/*.lst`

`arith_bench` 目录里那些额外文件不是这条正式路径的一部分, 不应该作为现在文档和工具的依据.

## 先跑自动分析

推荐先直接跑:

```bash
just analyze-objects ad_da
```

如果你已经知道想找的名字, 可以加关键字:

```bash
just analyze-objects ad_da key
just analyze-objects arith_bench sink
```

这条命令会自动做几件事:

- 读取 `Listings/*.m51` 的最终链接结果.
- 提取适合 `peek_*` / `poke_*` 使用的固定地址符号.
- 根据段类型给出推荐接口.
- 额外搜索 `Listings/*.lst`, 帮你把声明位置和源码上下文也找出来.

## 手工排查时看什么

### `*.m51`

`m51` 是第一优先级.

它最适合看:

- 全局变量最终落在哪个地址.
- 变量属于 `data`, `idata`, `pdata`, `xdata`, 还是位地址.
- `sbit` 最终映射到哪个 `SFR` 位.
- 链接后的整体内存布局.

例如 `samples/ad_da/prj/Listings/ad_da.m51` 里可以直接看到:

- `I:000DH PUBLIC key_sd`
- `I:0011H PUBLIC ad_da_sd`
- `X:0000H PUBLIC sg_buf`
- `B:0020H.0 PUBLIC detect_rb`

这些已经足够直接写出:

```rhai
peek_idata(0x0D)
peek_idata(0x11)
peek_pdata(0x00)
peek_data(0x20) & 0x01
```

再比如 `samples/arith_bench/prj/Listings/arith_bench.m51` 里可以直接看到:

- `D:0036H PUBLIC sink_u8`
- `D:003DH PUBLIC sink_int`
- `B:00B0H.4 PUBLIC bench_pin`

对应就是:

```rhai
peek_data(0x36)
peek_data(0x3D)
peek_sfr(0xB0) & 0x10
```

### `*.lst`

`lst` 是第二优先级.

它最适合看:

- 变量在源码里是怎么声明的.
- 变量本来是 `idata`, `pdata`, `bit` 还是别的类型.
- 某个局部变量是不是被分到了寄存器, 栈, 或 overlay 区.

例如 `samples/ad_da/prj/Listings/main.lst` 里可以直接看到:

```c
idata u8 sg_sd;
idata u8 key_sd;
idata u8 ad_da_sd;
pdata u8 sg_buf[8] = { ... };
bit detect_rb = 1;
```

所以当 `m51` 告诉你 `sg_buf` 在 `X:0000H` 时, 你还可以从 `lst` 确认它其实是 `pdata`, 也就是应该优先用 `peek_pdata`, 而不是 `peek_xdata`.

如果 `lst` 里出现这类内容:

```text
Allocated to registers
Allocated to stack
```

那说明它不是稳定的绝对地址, 不适合直接写进长期 judge.

## 段类型和接口对照

| 段或符号类型 | 含义 | 推荐接口 |
| --- | --- | --- |
| `D:` 且地址 `< 0x80` | `data` | `peek_data`, `poke_data` |
| `I:` | `idata` | `peek_idata`, `poke_idata` |
| `X:` 且在 `INPAGE ?PD?...` | `pdata` | `peek_pdata`, `poke_pdata` |
| `X:` 其他情况 | `xdata` | `peek_xdata`, `poke_xdata` |
| `D:` 且地址 `>= 0x80` | `SFR` | `peek_sfr`, `peek_sfr_latch`, `poke_sfr` |
| `B:` 且基础地址 `< 0x80` | 位区 | 先读字节再做掩码 |
| `B:` 且基础地址 `>= 0x80` | `sbit` | 先读 `SFR` 再做掩码 |

位地址没有单独的 `peek_bit`.

例如:

```rhai
let detect_rb = (peek_data(0x20) & 0x01) != 0;
let bench_pin = (peek_sfr(0xB0) & 0x10) != 0;
```

## 推荐流程

### 1. 先跑 recipe

```bash
just analyze-objects ad_da
```

如果输出里已经有你要的符号和接口, 直接用就行.

### 2. 再看 `m51`

如果 recipe 结果还不够, 先去 `Listings/<target>.m51` 搜名字.

例如:

```bash
rg -n 'PUBLIC        key_sd|PUBLIC        sg_buf|PUBLIC        detect_rb' samples/ad_da/prj/Listings/ad_da.m51
```

### 3. 最后看 `lst`

用 `lst` 确认声明和局部分配.

例如:

```bash
rg -n 'idata u8 key_sd|pdata u8 sg_buf|bit detect_rb' samples/ad_da/prj/Listings/main.lst
rg -n 'Allocated to stack|Allocated to registers' samples/ad_da/prj/Listings/main.lst
```

## 怎样判断能不能写进长期 judge

适合长期 judge 的, 一般是:

- `m51` 里明确列出的 `PUBLIC` 全局变量.
- 明确的 `SFR`.
- 明确的 `pdata` 或 `xdata` 缓冲区.

不适合长期 judge 的, 一般是:

- `PROC` 下面的临时 `SYMBOL`.
- `Allocated to registers`.
- `Allocated to stack`.
- overlay 复用出来的短期地址.

## 一个实用判断

如果地址来自 `m51` 的 `PUBLIC`, 它通常就是最终地址, 可以直接作为 `peek_*` / `poke_*` 参考.

如果信息只出现在 `lst` 的局部分配说明里, 那它通常只适合临时调试, 不适合写死.
