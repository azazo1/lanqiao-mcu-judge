# arith_bench

这个样例用于测量不同数据类型在常见算术运算下的仿真耗时.

## 引脚约定

- `P3.4` 作为测量引脚.
- 每个 bench 阶段开始前拉高 `P3.4`.
- 当前阶段计算结束后拉低 `P3.4`.
- 相邻阶段之间保留固定低电平间隔, 方便 judge 用上下沿拆分各个阶段.

## 阶段顺序

程序会按下面顺序循环输出 38 个脉冲.

1. `u8_add`
2. `u8_sub`
3. `u8_mul`
4. `u8_div`
5. `u8_mod`
6. `char_add`
7. `char_sub`
8. `char_mul`
9. `char_div`
10. `char_mod`
11. `int_add`
12. `int_sub`
13. `int_mul`
14. `int_div`
15. `int_mod`
16. `uint_add`
17. `uint_sub`
18. `uint_mul`
19. `uint_div`
20. `uint_mod`
21. `long_add`
22. `long_sub`
23. `long_mul`
24. `long_div`
25. `long_mod`
26. `ulong_add`
27. `ulong_sub`
28. `ulong_mul`
29. `ulong_div`
30. `ulong_mod`
31. `float_add`
32. `float_sub`
33. `float_mul`
34. `float_div`
35. `double_add`
36. `double_sub`
37. `double_mul`
38. `double_div`

每个阶段都只覆盖一种运算符, 方便直接统计单位运算时间.

## 单位运算统计

- `u8_*`, `char_*`, `int_*`, `uint_*` 每个阶段执行 `96` 轮循环.
- `long_*`, `ulong_*` 每个阶段执行 `64` 轮循环.
- `float_*`, `double_*` 每个阶段执行 `96` 轮循环.
- 每轮固定执行 `4` 次同类运算.

因此每个阶段的总运算次数如下:

- `u8_*`, `char_*`, `int_*`, `uint_*`: `384` 次.
- `long_*`, `ulong_*`: `256` 次.
- `float_*`, `double_*`: `384` 次.

judge 会对两轮测量结果取平均, 输出汇总表, 并给出每个阶段的平均 `ns/op`.

在 Keil C51 下, `double` 可能与 `float` 共用实现, 因此两者结果可能接近.

## 评测思路

- judge 先等待 `P3.4` 的首个上升沿.
- 每个高电平脉宽对应一个 bench 阶段的仿真耗时.
- judge 会对齐两轮完整阶段序列, 检查阶段顺序, 脉宽可测, 低电平间隔存在, 以及同一阶段的两轮耗时保持接近.
- judge 会写入 `add_marker(...)`, 方便在波形图中定位每个阶段的起止位置.
- judge 会在脚本末尾统一输出中文统计表, 避免运行过程中逐条打印导致输出分散.
