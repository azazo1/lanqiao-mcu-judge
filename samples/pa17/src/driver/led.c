#include "led.h" // 包含LED及其他外设相关的头文件

// --- LED 显示相关全局变量 ---
// 使用 "idata" 关键字将变量存储在8051的内部RAM中，访问速度更快。
// temp_1 用于存储当前想要显示的LED状态
idata unsigned char temp_1 = 0x00;
// temp_1_old 用于缓存上一次写入硬件的LED状态，用于判断状态是否变化
idata unsigned char temp_1_old = 0xff;

/**
 * @brief LED显示函数
 * @param ucLed 一个指向8元素数组的指针，数组每个元素为0或1，分别对应8个LED的灭和亮。
 * 例如 ucLed[0] 控制 LED1, ucLed[7] 控制 LED8。
 */
void Led_Disp(unsigned char *ucLed)
{
	unsigned char temp; // 用于P2口操作的临时变量

	// 1. 将8个独立的0/1值合并成一个8位的字节(temp_1)
	//    例如，如果ucLed = {1,0,1,0,0,0,0,0}, temp_1 将变为 00000101b = 0x05
	temp_1 = 0x00; // 先清零
	temp_1 = (ucLed[0] << 0) | (ucLed[1] << 1) | (ucLed[2] << 2) | (ucLed[3] << 3) |
			 (ucLed[4] << 4) | (ucLed[5] << 5) | (ucLed[6] << 6) | (ucLed[7] << 7);

	// 2. 状态改变检测：只有当期望的LED状态与上一次设置的状态不同时，才执行硬件操作
	if (temp_1 != temp_1_old)
	{
		// 3. 更新硬件
		// P0口写入temp_1的反码。因为LED是共阳极连接，低电平(0)点亮，高电平(1)熄灭。
		// 例如，要点亮LED1(ucLed[0]=1), temp_1的bit0为1, ~temp_1的bit0则为0，输出低电平点亮LED。
		P0 = ~temp_1;

		// 使用138译码器选择LED外设组（Y4输出，地址100）
		temp = P2 & 0x1f;	// 保留P2低5位
		temp = temp | 0x80; // 设置P2高3位为100
		P2 = temp;			// 选中LED锁存器
		temp = P2 & 0x1f;	// 恢复P2高3位为000
		P2 = temp;			// 取消选择，完成数据锁存

		// 4. 更新状态缓存
		temp_1_old = temp_1;
	}
}
