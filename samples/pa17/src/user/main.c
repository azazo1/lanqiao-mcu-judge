// 包含 STC15F2K60S2 单片机的头文件，提供了寄存器定义等
#include <STC15F2K60S2.H>

// 包含自定义的初始化、外设驱动和功能模块的头文件
#include "init.h" // 系统初始化
#include "led.h"  // LED 模块
#include "seg.h"  // 数码管模块
#include "key.h"  // 按键模块

#include "ds1302.h" // DS1302 实时时钟模块
#include "iic.h"	// I2C 协议，用于 ADC/DAC 和 EEPROM
#include "uart.h"	// 串口通信模块

// 包含标准库头文件
#include "string.h" // 字符串处理函数
#include "stdio.h"	// 标准输入输出函数，如 printf 和 sscanf

// 全局变量定义

// `idata` 存储在内部高速 RAM 中，访问速度快
// 系统滴答计时器，由 Timer1 中断每毫秒增加一次
idata unsigned long int uwTick;

// `pdata` 存储在外部 RAM 的分页区域，访问速度较快
// LED灯的状态数组，对应8个LED的亮灭
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};

// 数码管显示缓冲区，存储要在8位数码管上显示的数字或符号
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10}; // 10代表不显示
// 当前正在刷新的数码管位置索引 (0-7)
idata unsigned char Seg_Pos = 0;

// 按键状态变量
idata unsigned char Key_Val;  // 当前按键值
idata unsigned char Key_Old;  // 上一次的按键值，用于检测边沿
idata unsigned char Key_Up;	  // 按键抬起事件标志
idata unsigned char Key_Down; // 按键按下事件标志

// `pdata` 存储在外部 RAM 的分页区域
// RTC实时时钟数据数组 [时, 分, 秒]
pdata unsigned char ucRtc[3] = {11, 12, 13};

// 串口接收相关变量
idata unsigned char Uart_Rx_Index;									  // 串口接收缓冲区索引
pdata unsigned char Uart_Rx_Buf[10] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0}; // 串口接收缓冲区
idata unsigned char Uart_Rx_Flag;									  // 串口接收到数据的标志
idata unsigned char Uart_Rx_Tick;									  // 串口接收超时计时器

idata unsigned char Seg_Show_Mode = 0; // 0 运行界面 1 地址设置界面 2 泄压记录界面
bit Factory_Mode = 0;				   // 工厂模式
bit Calibration_Interface = 0;		   // 校准界面标志0 PH 1 PL

idata unsigned char Pressure_Value_10x = 0; // 当前压力值10倍
idata unsigned char Device_Address = 0;		// 设备地址
bit Pressure_Relief_Flag = 0;				// 泄压事件发生标志

pdata unsigned char Pressure_Relief_Flag_ucRtc[3] = {0, 0, 0}; // 最近一次泄压事件发生的时间 [时,分]

idata unsigned char PH_10x = 0, PL_10x = 0;			  // 满量程校准值，零点校准值
idata unsigned char PH_10x_Ctrl = 0, PL_10x_Ctrl = 0; // 满量程校准值控制值，零点校准值控制值

idata unsigned int time_3s_factory = 0; // 工厂模式切换 3s定时器
bit time_3s_factory_flag = 0;			// 工厂模式切换 3s定时器标志
bit time_3s_factory_flag_running = 0;	// 工厂模式切换 是否已经运行过了

bit Calibration_Value_Change_Flag = 0; // 校准值改变屏 蔽泄压 标志
idata unsigned int time_3s_cal = 0;	   // 校准值改变 屏蔽泄压 3s定时器

idata unsigned int time_3s_reset = 0; // 复位标志 3s定时器
bit Reset_Flag = 0;					  // 复位标志
bit Reset_Flag_Running = 0;			  // 复位标志是否已经运行过了

idata unsigned char Pressure_Value_10x_Buf[21] = {0};
idata unsigned char Pressure_Value_10x_Buf_Index = 0;
bit Pressure_Vlaue_10x_Buf_Full = 0; // 压力值缓冲区满标志
bit Leakage_Lock_Flag = 0;			 // 泄漏锁标志

bit Led_Uart_Flag = 0;					 // 通信LED闪烁
idata unsigned char time_200ms_uart = 0; // 通信LED闪烁定时器
bit Led_Uart_Receive_Flag = 0;			 // 通信LED接收标志
idata unsigned int time_3s_uart = 0;	 // 通信LED接受定时器

bit Led_Factory_Flag = 0;					// 工厂模式LED闪烁
idata unsigned char time_200ms_factory = 0; // 工厂模式LED闪烁定时器

void Key_Proc()
{
	Key_Val = Key_Read(); // 读取当前按key的状态
	// 使用异或运算检测按键状态变化，& Key_Val 检测下降沿（按下）
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	// 使用异或运算检测按键状态变化，& ~Key_Val 检测上降沿（抬起）
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val; // 更新旧的按键值
	if (Key_Down == 5)
	{
		time_3s_factory = 0;			  // 工厂模式切换 3s定时器清零
		time_3s_factory_flag = 1;		  // 工厂模式切换 3s定时器启动
		time_3s_factory_flag_running = 0; // 工厂模式切换 是否已经运行过了
	}
	if (time_3s_factory >= 3000)
	{
		// 如果没有执行过工厂模式切换
		if (time_3s_factory_flag_running == 0)
		{
			time_3s_factory_flag_running = 1; // 工厂模式切换 3s定时器已经运行过了
			// 退出工厂模式
			if (Factory_Mode)
			{
				Factory_Mode = 0;
				Calibration_Value_Change_Flag = (PH_10x != PH_10x_Ctrl ||
												 PL_10x != PL_10x_Ctrl);
				time_3s_cal = 0; // 校准值改变 3s定时器归零
				PH_10x = PH_10x_Ctrl;
				PL_10x = PL_10x_Ctrl;
			}
			// 进入工厂模式
			else
			{
				Factory_Mode = 1;
				Calibration_Interface = 0; // 进入工厂模式时，默认进入PH校准界面
				PH_10x_Ctrl = PH_10x;
				PL_10x_Ctrl = PL_10x;
			}
		}
	}
	if (Key_Up == 5)
	{
		if (time_3s_factory < 3000)
		{
			if (Factory_Mode)
			{
				// 工厂模式
				Calibration_Interface = ~Calibration_Interface; // 切换校准界面
			}
		}
		time_3s_factory = 0;
		time_3s_factory_flag = 0; // 工厂模式切换 3s定时器停止
		time_3s_factory_flag_running = 0;
	}
	if (Key_Down == 13)
	{
		time_3s_reset = 0;		// 复位 3s定时器归零
		Reset_Flag = 1;			// 复位标志
		Reset_Flag_Running = 0; // 是否启动过复位标志
	}

	if (time_3s_reset >= 3000)
	{
		if (Reset_Flag_Running == 0)
		{
			Reset_Flag_Running = 1;
			Device_Address = 8;
			// 工厂模式的时候，如果复位，也得跟着复位
			PH_10x = PH_10x_Ctrl = 90;
			PL_10x = PL_10x_Ctrl = 0;
			EEPROM_Write(&Device_Address, 0x03, 1); // 将设备地址写入EEPROM的地址0x03
			EEPROM_Write(&PH_10x, 0x02, 1);			// 将PH_10x写入EEPROM的地址0x02
			EEPROM_Write(&PL_10x, 0x01, 1);			// 将PL_10x写入EEPROM的地址0x01
			Factory_Mode = 0;						// 退出工厂模式
			Seg_Show_Mode = 0;						// 显示模式切换到运行界面
		}
	}
	if (Key_Up == 13)
	{
		if (time_3s_reset < 3000)
		{
			// 在泄压界面下
			if ((Factory_Mode == 0) && (Seg_Show_Mode == 2))
			{
				// 清除泄压事件记录
				Pressure_Relief_Flag_ucRtc[0] = 0;
				Pressure_Relief_Flag_ucRtc[1] = 0;
				Pressure_Relief_Flag_ucRtc[2] = 0;
				Pressure_Relief_Flag = 0;
			}
		}
		time_3s_reset = 0;
		Reset_Flag = 0;			// 复位标志停止
		Reset_Flag_Running = 0; // 是否启动过复位标志
	}
	// 不在工厂模式
	if (Factory_Mode == 0)
	{

		switch (Key_Down)
		{
		case 4:
			Seg_Show_Mode = (Seg_Show_Mode + 1) % 3; // 切换显示模式
			break;

		case 8:
			if (Seg_Show_Mode == 1)
			{
				if (Device_Address > 1)
					Device_Address = Device_Address - 1; // 设备地址递减
			}
			break;
		case 9:
			if (Seg_Show_Mode == 1)
			{
				if (Device_Address < 100)
					Device_Address = Device_Address + 1; // 设备地址递增
			}
			break;
		case 12:
			if (Seg_Show_Mode == 1)
			{
				EEPROM_Write(&Device_Address, 0x03, 1); // 将设备地址写入EEPROM的地址0x03
			}
			break;
		}
	}
	else
	{
		switch (Key_Down)
		{
		case 8:
			if (Calibration_Interface == 0)
			{
				if (PH_10x_Ctrl > 0)
					PH_10x_Ctrl = PH_10x_Ctrl - 1; // PH校准值递减
			}
			else
			{
				if (PL_10x_Ctrl > 0)
					PL_10x_Ctrl = PL_10x_Ctrl - 1; // PL校准值递减
			}
			break;
		case 9:
			if (Calibration_Interface == 0)
			{
				if (PH_10x_Ctrl < 99)
					PH_10x_Ctrl = PH_10x_Ctrl + 1; // PH校准值递增
			}
			else
			{
				if (PL_10x_Ctrl < 99)
					PL_10x_Ctrl = PL_10x_Ctrl + 1; // PL校准值递增
			}
			break;
		case 12:
			if (Calibration_Interface == 0)
			{
				EEPROM_Write(&PH_10x_Ctrl, 0x02, 1); // 将设备地址写入EEPROM的地址0x02
			}
			else
			{
				EEPROM_Write(&PL_10x_Ctrl, 0x01, 1); // 将设备地址写入EEPROM的地址0x01
			}
			break;
		}
	}
}

void Seg_Proc()
{
	if (Factory_Mode)
	{
		// 工厂模式
		Seg_Buf[0] = 16; // P
		if (Calibration_Interface == 0)
		{
			Seg_Buf[1] = 14; // H
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = 17; //-
			Seg_Buf[6] = PH_10x_Ctrl / 10 % 10 + ',';
			Seg_Buf[7] = PH_10x_Ctrl % 10;
		}
		else
		{
			Seg_Buf[1] = 15; // L
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = 17; //-
			Seg_Buf[6] = PL_10x_Ctrl / 10 % 10 + ',';
			Seg_Buf[7] = PL_10x_Ctrl % 10;
		}
	}
	else
	{
		switch (Seg_Show_Mode)
		{
		case 0:
			/* 运行界面 */
			Seg_Buf[0] = ucRtc[0] / 10;
			Seg_Buf[1] = ucRtc[0] % 10;
			Seg_Buf[2] = ucRtc[1] / 10;
			Seg_Buf[3] = ucRtc[1] % 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = 10;
			Seg_Buf[6] = Pressure_Value_10x / 10 % 10 + ',';
			Seg_Buf[7] = Pressure_Value_10x % 10;
			break;
		case 1:
			/* 地址设置界面 */
			Seg_Buf[0] = 11; // A
			Seg_Buf[1] = 13; // E
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = (Device_Address >= 100)
							 ? Device_Address / 100 % 10
							 : 10;
			Seg_Buf[6] = (Device_Address >= 10)
							 ? Device_Address / 10 % 10
							 : 10;
			Seg_Buf[7] = Device_Address % 10;
			break;
		case 2:
			/* 泄压记录界面 */
			Seg_Buf[0] = 12; // C
			Seg_Buf[1] = 12; // C
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			if (Pressure_Relief_Flag)
			{
				// 发生泄压事件，最近一次的时间
				Seg_Buf[4] = Pressure_Relief_Flag_ucRtc[0] / 10;
				Seg_Buf[5] = Pressure_Relief_Flag_ucRtc[0] % 10;
				Seg_Buf[6] = Pressure_Relief_Flag_ucRtc[1] / 10;
				Seg_Buf[7] = Pressure_Relief_Flag_ucRtc[1] % 10;
			}
			else
			{

				Seg_Buf[4] = 10;
				Seg_Buf[5] = 10;
				Seg_Buf[6] = 10;
				Seg_Buf[7] = 17; //-
			}
			break;
		}
	}
}

void Led_Proc()
{
	ucLed[0] = Led_Uart_Flag;
	ucLed[1] = Led_Factory_Flag;
	ucLed[2] = Pressure_Relief_Flag;
	Led_Disp(ucLed);
}

void Get_Time()
{
	Read_Rtc(ucRtc);
}

void AD_Proc()
{
	unsigned char ad_value = 0;
	int pressure_now = 0;
	int pressure_old = 0;
	int diff3s = 0;
	bit leak_cond_now = 0;	  // 当前泄压情况
	ad_value = Ad_Read(0x03); // RB2

	// k=(PH-PL)/(255-0)=(PH-PL)/255,b=PL
	// y=kx+b=(PH-PL)*ad_value/255+PL
	Pressure_Value_10x = (PH_10x - PL_10x) * ad_value / 255 + PL_10x;

	pressure_now = Pressure_Value_10x;
	Pressure_Value_10x_Buf[Pressure_Value_10x_Buf_Index] = Pressure_Value_10x;

	if (Pressure_Vlaue_10x_Buf_Full)
	{
		pressure_old = Pressure_Value_10x_Buf[(Pressure_Value_10x_Buf_Index + 1) % 21];
		diff3s = pressure_now - pressure_old;
		leak_cond_now = (diff3s < -15); // 3s时压力下降超过15，则认为泄压
		// 如果是非屏蔽期，且当前处于泄压，且没有被锁定（上一次没有泄压）
		if ((!Calibration_Value_Change_Flag) &&
			(leak_cond_now) &&
			(!Leakage_Lock_Flag))
		{
			Leakage_Lock_Flag = 1;
			Pressure_Relief_Flag = 1;
			Read_Rtc(Pressure_Relief_Flag_ucRtc); // 记录泄压事件发生的时间
		}
		// 处于泄露锁，但是本次没有泄露，则解除泄露锁，防止连续泄露导致的持续记录
		if ((Leakage_Lock_Flag) && (!leak_cond_now))
		{
			Leakage_Lock_Flag = 0; // 解除泄漏锁
		}
	}

	Pressure_Value_10x_Buf_Index++;
	if (Pressure_Value_10x_Buf_Index >= 21)
	{
		Pressure_Value_10x_Buf_Index = 0;
		Pressure_Vlaue_10x_Buf_Full = 1;
	}
}
void Uart_Proc()
{
	unsigned int id;
	int rev = 0;
	int n = 0;
	// 如果没有接收到数据，直接返回
	if (Uart_Rx_Index == 0)
		return;

	// 如果距离上次接收到数据超过10ms（超时）
	if (Uart_Rx_Tick >= 10)
	{
		Uart_Rx_Flag = 0; // 清除接收标志
		Uart_Rx_Tick = 0; // 复位超时计时器
		// sscanf的返回值是成功匹配的参数个数，用uint接受，是因为可能会出现一个比如500这种超出uchar的，直接截断可能导致异常
		// 错误示例
		// #500?->直接超出范围，用uchar接受会被直接卡下来0001 1111 0100->1111 0100->244，导致解析错误
		// ####->没有成功解析到数字
		// #50?->成功解析到数字50，但字符个数不正确
		rev = sscanf((char *)Uart_Rx_Buf, "#%u?%n", &id, &n); // 解析接收到的数据，提取设备地址

		// 如果参数个数正确，且字符个数正确，且id在范围内
		if ((rev == 1) && (n == 5) && (id == Device_Address))
		{

			Led_Uart_Receive_Flag = 1;
			time_3s_uart = 3000; // 通信LED接受定时器设置为3s
			Led_Uart_Flag = 0;
			time_200ms_uart = 0;

			printf("%bu.%bukPa@%.2bu:%.2bu",
				   Pressure_Value_10x / 10,
				   Pressure_Value_10x % 10,
				   Pressure_Relief_Flag_ucRtc[0],
				   Pressure_Relief_Flag_ucRtc[1]);
		}
		// 清空接收缓冲区，为下次接收做准备
		memset(Uart_Rx_Buf, 0, Uart_Rx_Index);
		Uart_Rx_Index = 0;
	}
}

void Timer1_Init(void) // 1毫秒@12.000MHz
{
	AUXR &= 0xBF; // 定时器时钟12T模式
	TMOD &= 0x0F; // 设置定时器模式 (清空T1的设置)
	TL1 = 0x18;	  // 设置定时初始值 (65536-1000) -> FC18H
	TH1 = 0xFC;	  // 设置定时初始值
	TF1 = 0;	  // 清除TF1溢出标志
	TR1 = 1;	  // 定时器1开始计时
	ET1 = 1;	  // 使能定时器1中断
	EA = 1;		  // 开启总中断
}

void Timer1_Isr(void) interrupt 3
{
	uwTick++; // 系统滴答计时器加1

	// 数码管动态扫描
	Seg_Pos = (++Seg_Pos) % 8; // 切换到下一位数码管
	// 判断显示缓冲区的值是否需要显示小数点
	// (通过给数字加上一个大偏移量','作为标志)
	if (Seg_Buf[Seg_Pos] > 20)
		// 显示数字，并点亮小数点
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos] - ',', 1);
	else
		// 显示数字，不点亮小数点
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos], 0);
	// 如果串口正在接收数据，启动超时计时器
	if (Uart_Rx_Flag)
		Uart_Rx_Tick++;
	if (time_3s_factory_flag)
	{
		if (++time_3s_factory >= 3000)
		{
			time_3s_factory = 3001;
		}
	}
	else
	{
		time_3s_factory = 0;
	}
	if (Calibration_Value_Change_Flag)
	{
		if (++time_3s_cal >= 3000)
		{
			time_3s_cal = 0;
			Calibration_Value_Change_Flag = 0; // 超时后清除标志
		}
	}
	else
	{
		time_3s_cal = 0;
	}
	if (Reset_Flag)
	{
		if (++time_3s_reset >= 3000)
		{
			time_3s_reset = 3001;
		}
	}
	else
	{
		time_3s_reset = 0;
	}
	// 接收到通信3s内
	if (Led_Uart_Receive_Flag)
	{
		if (--time_3s_uart > 0)
		{
			if (++time_200ms_uart >= 200)
			{
				time_200ms_uart = 0;
				Led_Uart_Flag ^= 1; // 200ms翻转一次
			}
		}
		// 3s到达，关闭通信灯闪烁
		else
		{
			Led_Uart_Receive_Flag = 0;
			time_200ms_uart = 0;
			Led_Uart_Flag = 0;
		}
	}
	// 如果处于工厂模式
	if (Factory_Mode)
	{
		if (++time_200ms_factory >= 200)
		{
			time_200ms_factory = 0;
			Led_Factory_Flag ^= 1; // 200ms翻转一次
		}
		else
		{
			time_200ms_factory = 0;
			Led_Factory_Flag = 0;
		}
	}
}
void Uart1_Isr(void) interrupt 4
{
	if (RI) // 判断是否是接收中断
	{
		Uart_Rx_Flag = 1; // 设置接收标志
		Uart_Rx_Tick = 0; // 重置超时计时器
		// 将接收到的数据存入缓冲区
		Uart_Rx_Buf[Uart_Rx_Index++] = SBUF;
		RI = 0; // 手动清除接收中断请求位
		// 防止缓冲区溢出
		if (Uart_Rx_Index >= 10)
		{
			Uart_Rx_Index = 0;
			memset(Uart_Rx_Buf, 0, 10); // 溢出则清空缓冲区
		}
	}
}

// 简单任务调度器结构体定义
typedef struct
{
	void (*task_func)(void);   // 任务函数指针
	unsigned long int rate_ms; // 任务执行周期（毫秒）
	unsigned long int last_ms; // 上次执行的时间戳
} task_t;

// 任务列表
idata task_t Scheduler_Task[] = {
	{Led_Proc, 1, 0},	// LED任务，每1ms
	{Key_Proc, 10, 0},	// 按键任务，每10ms
	{Seg_Proc, 20, 0},	// 数码管任务，每20ms
	{Get_Time, 100, 0}, // 获取时间任务，每100ms
	{AD_Proc, 150, 0},	// AD/DA任务，每150ms
	{Uart_Proc, 10, 0}, // 串口处理任务，每10ms
};

idata unsigned char task_num; // 任务数量

void Scheduler_Init()
{
	task_num = sizeof(Scheduler_Task) / sizeof(task_t);
}

/**
 * @brief 调度器运行函数
 * 循环检查每个任务是否到达执行时间。
 */
void Scheduler_Run()
{
	unsigned char i;
	for (i = 0; i < task_num; i++)
	{
		unsigned long int now_time = uwTick; // 获取当前时间
		// 判断是否到达任务执行时间 (当前时间 >= 上次执行时间 + 周期)
		if (now_time >= (Scheduler_Task[i].rate_ms + Scheduler_Task[i].last_ms))
		{
			Scheduler_Task[i].last_ms = now_time; // 更新上次执行时间
			Scheduler_Task[i].task_func();		  // 执行任务函数
		}
	}
}

void main()
{
	unsigned char temp;
	unsigned char temp_buf[3] = {0};
	System_Init(); // 系统底层初始化

	EEPROM_Read(&temp, 0x00, 1); // 从EEPROM读取设备特征字
	if (temp == 0xaa)
	{
		EEPROM_Read(temp_buf, 0x01, 3); // 从EEPROM的地址0x00读取设备特征字、PH_10x、PL_10x
		PL_10x = temp_buf[0];
		PH_10x = temp_buf[1];
		Device_Address = temp_buf[2];
	}
	else
	{
		// 如果设备特征字不为0xaa，表示设备第一次使用，进行初始化设置
		Device_Address = 8;						// 默认设备地址
		PH_10x = 90;							// 默认PH满量程校准值
		PL_10x = 0;								// 默认PL零点校准值
		EEPROM_Write(&Device_Address, 0x03, 1); // 将设备地址写入EEPROM的地址0x03
		EEPROM_Write(&PH_10x, 0x02, 1);			// 将PH_10x写入EEPROM的地址0x02
		EEPROM_Write(&PL_10x, 0x01, 1);			// 将PL_10x写入EEPROM的地址0x01
		temp = 0xaa;
		EEPROM_Write(&temp, 0x00, 1); // 将设备特征字写入EEPROM的地址0x00
	}
	Set_Rtc(ucRtc);	  // 设置RTC时间
	Scheduler_Init(); // 任务调度器初始化
	Uart1_Init();	  // 串口1初始化
	Timer1_Init();	  // 定时器1（系统心跳）初始化

	// 主循环
	while (1)
	{
		Scheduler_Run(); // 循环执行任务调度器
	}
}