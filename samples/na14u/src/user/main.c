#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"
#include "ultrasound.h"
#include "filtering.h"
#include "onewire.h"
#include "intrins.h"

/* 变量 */
idata unsigned long int uwTick = 0; // 定时器
// 按键
idata unsigned char Key_Val, Key_Old, Key_Down, Key_Up;
// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
// 数码管
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata unsigned char Seg_Pos = 0;

idata unsigned char Seg_Show_Mode = 0;	   // 显示界面 0 测距 1 参数 2 工厂
idata bit Distance_Show_Mode = 0;		   // 距离显示模式 0 cm 1 m
idata unsigned char Para_Show_Mode = 0;	   // 参数界面 0 距离 1 温度
idata unsigned char Factory_Show_Mode = 0; // 工厂模式 0 校准值 1 介质 2 DAC输出

idata unsigned int Temperature_Value_10x = 0; // 温度测量结果十倍
idata unsigned char Distance_Value = 0;		  // 测距结果

idata unsigned char Distance_Para = 40;		   // 距离参数10~90
idata unsigned char Temperature_Para = 30;	   // 温度参数0~80
idata unsigned char Distance_Para_Ctrl = 0;	   // 距离参数控制值10~90
idata unsigned char Temperature_Para_Ctrl = 0; // 温度参数控制值0~80

idata char Calibration_Value = 0;				  // 校准值 -90~90
idata unsigned int Speed_Value = 340;			  // 传输速度 10~9990
idata unsigned char Dac_Limit_Value_10x = 10;	  // DAC下限的十倍 1~20
idata char Calibration_Value_Ctrl = 0;			  // 校准值控制值 -90~90
idata unsigned int Speed_Value_Ctrl = 0;		  // 传输速度控制值 10~9990
idata unsigned char Dac_Limit_Value_Ctrl_10x = 0; // DAC下限控制值的十倍 1~20

pdata unsigned char Distance_Arr[12] = {0}; // 测距结果存储数组
idata unsigned char Distance_Arr_Index = 0; // 测距结果存储数组下标
idata bit Record_Flag = 0;					// 当前是否记录 0 未记录 1 记录中
idata unsigned int Time_6s_Record = 0;		// 记录6s
idata bit Dac_Flag = 0;						// DAC开始输出
idata unsigned char Dac_Arr_Index = 0;		// DAC输出的下标
idata unsigned int Time_500Ms_Dac = 0;		// DAC输出一个数据的间隔时间（和超声波采样时间对应）

idata unsigned int Time_2s_Press = 0; // 长按计时
idata bit Long_Press_Flag = 0;		  // 长按检测

idata unsigned char Time_100Ms_Factory = 0; // 工厂模式100ms计时
idata bit Led_Light_Factory = 0;			// 工厂模式下LED的闪烁标志

/* 按键 */
void Key_Proc()
{
	Key_Val = Key_Read();
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val;
	// 当处于采集状态的时候，我们直接返回，按键全部失效
	if (Record_Flag)
		return;
	if (Key_Down == 89)
		Long_Press_Flag = 1;
	if (Time_2s_Press >= 2000)
	{
		Seg_Show_Mode = 0;
		Distance_Show_Mode = 0;
		Distance_Para = 40;
		Temperature_Para = 30;
		Calibration_Value = 0;
		Speed_Value = 340;
		Dac_Limit_Value_10x = 10;
		Long_Press_Flag = 0;
	}
	if (Key_Up == 89)
		Long_Press_Flag = 0;
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 测距界面 */
		if (Key_Down == 4)
		{
			Seg_Show_Mode = 1;
			Para_Show_Mode = 0;
			Temperature_Para_Ctrl = Temperature_Para;
			Distance_Para_Ctrl = Distance_Para;
		}
		if (Key_Down == 5)
			Distance_Show_Mode ^= 1;
		if (Key_Down == 8)
			Record_Flag = 1;
		if (Key_Down == 9)
			Dac_Flag = 1;
		break;
	case 1:
		if (Key_Down == 4)
		{
			Seg_Show_Mode = 2;
			Factory_Show_Mode = 0;
			Temperature_Para = Temperature_Para_Ctrl;
			Distance_Para = Distance_Para_Ctrl;

			Calibration_Value_Ctrl = Calibration_Value;
			Speed_Value_Ctrl = Speed_Value;
			Dac_Limit_Value_Ctrl_10x = Dac_Limit_Value_10x;
		}
		if (Key_Down == 5)
			Para_Show_Mode = (++Para_Show_Mode) % 2;
		/* 参数界面 */
		switch (Para_Show_Mode)
		{
		case 0:
			/* 距离参数 */
			if (Key_Down == 8)
				Distance_Para_Ctrl = (Distance_Para_Ctrl == 90)
										 ? 10
										 : Distance_Para_Ctrl + 10;
			if (Key_Down == 9)
				Distance_Para_Ctrl = (Distance_Para_Ctrl == 10)
										 ? 90
										 : Distance_Para_Ctrl - 10;
			break;
		case 1:
			/* 温度参数 */
			if (Key_Down == 8)
				Temperature_Para_Ctrl = (Temperature_Para_Ctrl == 80)
											? 0
											: Temperature_Para_Ctrl + 1;
			if (Key_Down == 9)
				Temperature_Para_Ctrl = (Temperature_Para_Ctrl == 0)
											? 80
											: Temperature_Para_Ctrl - 1;
			break;
		}
		break;
	case 2:
		/* 工厂界面 */
		if (Key_Down == 4)
		{
			Seg_Show_Mode = 0;
			Distance_Show_Mode = 0;
			Calibration_Value = Calibration_Value_Ctrl;
			Speed_Value = Speed_Value_Ctrl;
			Dac_Limit_Value_10x = Dac_Limit_Value_Ctrl_10x;
		}
		if (Key_Down == 5)
			Factory_Show_Mode = (++Factory_Show_Mode) % 3;
		switch (Factory_Show_Mode)
		{
		case 0:
			/* 校准值 */
			if (Key_Down == 8)
				Calibration_Value_Ctrl = (Calibration_Value_Ctrl == 90)
											 ? -90
											 : Calibration_Value_Ctrl + 5;
			if (Key_Down == 9)
				Calibration_Value_Ctrl = (Calibration_Value_Ctrl == -90)
											 ? 90
											 : Calibration_Value_Ctrl - 5;
			break;
		case 1:
			/* 传输速度 */
			if (Key_Down == 8)
				Speed_Value_Ctrl = (Speed_Value_Ctrl == 9990)
									   ? 10
									   : Speed_Value_Ctrl + 10;
			if (Key_Down == 9)
				Speed_Value_Ctrl = (Speed_Value_Ctrl == 10)
									   ? 9990
									   : Speed_Value_Ctrl - 10;
			break;
		case 2:
			/* DAC下限 */
			if (Key_Down == 8)
				Dac_Limit_Value_Ctrl_10x = (Dac_Limit_Value_Ctrl_10x == 20)
											   ? 1
											   : Dac_Limit_Value_Ctrl_10x + 1;
			if (Key_Down == 9)
				Dac_Limit_Value_Ctrl_10x = (Dac_Limit_Value_Ctrl_10x == 1)
											   ? 20
											   : Dac_Limit_Value_Ctrl_10x - 1;
			break;
		}
		break;
	}
}

/* 数码管 */
void Seg_Proc()
{
	unsigned char Temp_Calibration_Value_Ctrl = -Calibration_Value_Ctrl;
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 测距界面 */
		Seg_Buf[0] = Temperature_Value_10x / 100;
		Seg_Buf[1] = Temperature_Value_10x / 10 % 10 + ',';
		Seg_Buf[2] = Temperature_Value_10x % 10;
		Seg_Buf[3] = 11; //-
		Seg_Buf[4] = 10;
		// 单位为cm
		if (Distance_Show_Mode == 0)
		{
			Seg_Buf[5] = (Distance_Value / 100 == 0)
							 ? 10
							 : Distance_Value / 100;
			Seg_Buf[6] = ((Distance_Value / 10 % 10 == 0) && (Seg_Buf[5] == 10))
							 ? 10
							 : Distance_Value / 10 % 10;
			Seg_Buf[7] = Distance_Value % 10;
		}
		// 单位为m
		else
		{
			Seg_Buf[5] = Distance_Value / 100 + ',';
			Seg_Buf[6] = Distance_Value / 10 % 10;
			Seg_Buf[7] = Distance_Value % 10;
		}
		break;
	case 1:
		/* 参数界面 */
		Seg_Buf[0] = 12; // P
		Seg_Buf[1] = Para_Show_Mode + 1;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		switch (Para_Show_Mode)
		{
		case 0:
			/* 距离参数 */
			Seg_Buf[6] = Distance_Para_Ctrl / 10 % 10;
			Seg_Buf[7] = Distance_Para_Ctrl % 10;
			break;
		case 1:
			/* 温度参数 */
			Seg_Buf[6] = Temperature_Para_Ctrl / 10 % 10;
			Seg_Buf[7] = Temperature_Para_Ctrl % 10;

			break;
		}
		break;
	case 2:
		/* 工厂界面 */
		Seg_Buf[0] = 13; // F
		Seg_Buf[1] = Factory_Show_Mode + 1;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		switch (Factory_Show_Mode)
		{
		case 0:
			/* 校准值 */
			Seg_Buf[4] = 10;
			if (Calibration_Value_Ctrl <= -10)
			{
				Seg_Buf[5] = 11; //-
				Seg_Buf[6] = Temp_Calibration_Value_Ctrl / 10;
				Seg_Buf[7] = Temp_Calibration_Value_Ctrl % 10;
			}
			else if (Calibration_Value_Ctrl < 0)
			{
				Seg_Buf[5] = 10;
				Seg_Buf[6] = 11; //-
				Seg_Buf[7] = Temp_Calibration_Value_Ctrl % 10;
			}
			else if (Calibration_Value_Ctrl < 10)
			{
				Seg_Buf[5] = 10;
				Seg_Buf[6] = 10;
				Seg_Buf[7] = Calibration_Value_Ctrl % 10;
			}
			else
			{
				Seg_Buf[5] = 10;
				Seg_Buf[6] = Calibration_Value_Ctrl / 10;
				Seg_Buf[7] = Calibration_Value_Ctrl % 10;
			}
			break;
		case 1:
			/* 传输速度 */
			Seg_Buf[4] = (Speed_Value_Ctrl / 1000 == 0)
							 ? 10
							 : Speed_Value_Ctrl / 1000;
			Seg_Buf[5] = ((Speed_Value_Ctrl / 100 % 10 == 0) && (Seg_Buf[4] == 10))
							 ? 10
							 : Speed_Value_Ctrl / 100 % 10;
			Seg_Buf[6] = ((Speed_Value_Ctrl / 10 % 10 == 0) && (Seg_Buf[5] == 10))
							 ? 10
							 : Speed_Value_Ctrl / 10 % 10;
			Seg_Buf[7] = 0;

			break;
		case 2:
			/* DAC下限 */
			Seg_Buf[4] = 10;
			Seg_Buf[5] = 10;
			Seg_Buf[6] = Dac_Limit_Value_Ctrl_10x / 10 + ',';
			Seg_Buf[7] = Dac_Limit_Value_Ctrl_10x % 10;
			break;
		}
		break;
	}
}

/* LED */
void Led_Proc()
{
	unsigned char i;
	bit temp;
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 测距界面 */
		for (i = 0; i < 8; i++)
		{
			ucLed[i] = (Distance_Value >> i) & 0x01;
		}
		break;
	case 1:
		/* 参数界面 */
		for (i = 0; i < 7; i++)
		{
			ucLed[i] = 0;
		}
		ucLed[7] = 1;
		break;
	case 2:
		/* 工厂界面 */
		for (i = 1; i < 8; i++)
		{
			ucLed[i] = 0;
		}
		ucLed[0] = Led_Light_Factory;
		break;
	}
	Led_Disp(ucLed);
	temp = ((Distance_Value >= Distance_Para - 5) &&
			(Distance_Value <= Distance_Para + 5) &&
			(Temperature_Value_10x <= Temperature_Para * 10));
	Relay(temp);
}

/* AD_DA */
void AD_DA()
{
	float temp_da;
	if (Dac_Arr_Index == 11)
	{
		Dac_Arr_Index = 0;
		Dac_Flag = 0;
	}
	// 当输出有效的时候，才进行输出
	if (Dac_Flag)
	{
		if (Distance_Arr[Dac_Arr_Index] <= 10)
			Da_Write(Dac_Limit_Value_10x * 51 / 10);
		else if (Distance_Arr[Dac_Arr_Index] >= 90)
			Da_Write(5 * 51);
		else
		{
			temp_da = 5 - (5 * 10 - Dac_Limit_Value_10x) * (float)(90 - Distance_Arr[Dac_Arr_Index]) / (80.0 * 10);
			Da_Write(temp_da * 51);
		}
	}
	else
		Da_Write(0 * 51);
}

/* 温度 */
void Get_Temperature()
{
	float temp;
	temp = rd_temperature();
	Temperature_Value_10x = Median_Filter(temp) * 10;
}

/* 距离 */
void Get_Distance()
{
	unsigned char temp;
	unsigned char Temp_Calibration_Value = -Calibration_Value;
	temp = Ut_Wave_Data(Speed_Value);
	if (Calibration_Value < 0)
	{
		if (temp > Temp_Calibration_Value)
			Distance_Value = temp - Temp_Calibration_Value;
	}
	else
	{
		if (temp != 0)
			Distance_Value = temp + Calibration_Value;
	}
	// 如果开始记录
	if (Record_Flag)
	{
		// 当数组未填满
		if (Distance_Arr_Index < 12)
		{
			Distance_Arr[Distance_Arr_Index] = Distance_Value;
			Distance_Arr_Index++;
		}
	}
	// 没有记录的时候，直接把下标清零
	else
		Distance_Arr_Index = 0;
}

/* 定时器 */
void Timer1_Init(void) // 1毫秒@12.000MHz
{
	AUXR &= 0xBF; // 定时器时钟12T模式
	TMOD &= 0x0F; // 设置定时器模式
	TL1 = 0x18;	  // 设置定时初始值
	TH1 = 0xFC;	  // 设置定时初始值
	TF1 = 0;	  // 清除TF1标志
	TR1 = 1;	  // 定时器1开始计时
	ET1 = 1;	  // 使能定时器1中断
	EA = 1;
}

void Timer1_Isr(void) interrupt 3
{
	uwTick++;
	Seg_Pos = (++Seg_Pos) % 8;
	if (Seg_Buf[Seg_Pos] > 20)
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos] - ',', 1);
	else
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos], 0);
	// 工厂界面闪烁
	if (Seg_Show_Mode == 2)
	{
		if (++Time_100Ms_Factory == 100)
		{
			Time_100Ms_Factory = 0;
			Led_Light_Factory ^= 1;
		}
	}
	else
	{
		Time_100Ms_Factory = 0;
		Led_Light_Factory = 0;
	}
	if (Long_Press_Flag)
	{
		if (++Time_2s_Press >= 2000)
			Time_2s_Press = 2001;
	}
	else
		Time_2s_Press = 0;
	if (Record_Flag)
	{
		if (++Time_6s_Record == 6000)
		{
			Time_6s_Record = 0;
			Record_Flag = 0;
		}
	}
	else
		Time_6s_Record = 0;
	if (Dac_Flag)
	{
		if (++Time_500Ms_Dac == 500)
		{
			Time_500Ms_Dac = 0;
			Dac_Arr_Index++;
		}
	}
	else
		Time_500Ms_Dac = 0;
}

/* 调度器 */
typedef struct
{
	void (*task_func)(void);   // 任务函数
	unsigned long int rate_ms; // 任务周期
	unsigned long int last_ms; // 最后一次任务时间
} task_t;

idata task_t Scheduler_Task[] =
	{
		{Led_Proc, 1, 0},
		{Key_Proc, 10, 0},
		{Seg_Proc, 90, 0},
		{AD_DA, 80, 0},
		{Get_Temperature, 300, 0},
		{Get_Distance, 460, 0}};

idata unsigned char task_num;

void Scheduler_Init()
{
	task_num = sizeof(Scheduler_Task) / sizeof(task_t);
}

void Scheduler_Run()
{
	unsigned char i;
	for (i = 0; i < task_num; i++)
	{
		unsigned long int now_time = uwTick;
		if (now_time >= Scheduler_Task[i].rate_ms + Scheduler_Task[i].last_ms)
		{
			Scheduler_Task[i].last_ms = now_time;
			Scheduler_Task[i].task_func();
		}
	}
}

void Delay750ms(void) //@12.000MHz
{
	unsigned char data i, j, k;

	_nop_();
	_nop_();
	i = 35;
	j = 51;
	k = 182;
	do
	{
		do
		{
			while (--k)
				;
		} while (--j);
	} while (--i);
}

void main()
{
	System_Init();
	rd_temperature();
	Delay750ms();
	Scheduler_Init();
	Timer1_Init();
	while (1)
	{
		Scheduler_Run();
	}
}