# LED 闪烁示例

在进行了系统初始化之后 LED L1 以 0.1s 为间隔切换亮灭状态.

这类固定节奏的亮灭切换, judge 更适合断言 `watch_led_stats(...).change_frequency_hz`, 目标约为 `10Hz`.
