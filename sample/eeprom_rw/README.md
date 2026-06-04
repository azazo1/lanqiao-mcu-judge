# EEPROM READ WRITE

仅仅在 0 地址发出 EEPROM WRITE 和 READ 信号, 并且添加一个可能的 Timer1 控制 P2 P0, 模拟干扰.

## 中断干扰 eeprom 读取情况

如果在 IIC 读取的时候不加中断屏蔽, 那么就可能导致 P2 P0 latch 写入的时候拉低.

![](error_isr_latch.png)

中断在 IIC 读取的时候强行插入, `P2 = P2 & 0x1f | 0x80; P2 &= 0x1f;` 这一步读取了拉低的 P20, 并且置位 P20 为 0, 导致 iic bus 被拉低, 无法读取到正确的数值.
