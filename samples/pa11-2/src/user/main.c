#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"
#include "onewire.h"
#include "intrins.h"
/* 变量 */
idata unsigned long int uwTick = 0; // 定时
// 按键
idata unsigned char Key_Val, Key_Old, Key_Down, Key_Up;
// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
// 数码管
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata unsigned char Seg_Pos = 0;

idata bit Seg_Show_Mode = 0;					 // 界面 0 数据显示 1 参数设置
idata bit Setting_Mode = 0;						 // 参数设置 0 设置温度下限 1 设置温度上限
idata unsigned char Temputure_Value = 0;		 // 测量结果
idata unsigned char Temputure_Para_Max = 30;	 // 温度参数上限
idata unsigned char Temputure_Para_Min = 20;	 // 温度参数下限
idata unsigned char Temputure_Para_Max_Ctrl = 0; // 温度参数上限控制
idata unsigned char Temputure_Para_Min_Ctrl = 0; // 温度参数下限控制
idata bit Error_Set = 0;						 // 错误的上下限设置

/* 按键 */
void Key_Proc()
{
	Key_Val = Key_Read();
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val;
	switch (Key_Down)
	{
	case 4:
		// 如果我现在是数据设置界面，界面切换，实际值给控制值，保证进入后是下限设置
		if (Seg_Show_Mode == 0)
		{
			Seg_Show_Mode = 1;
			Setting_Mode = 0;
			Temputure_Para_Max_Ctrl = Temputure_Para_Max;
			Temputure_Para_Min_Ctrl = Temputure_Para_Min;
		}
		// 如果我现在是参数设置界面，界面切换，合理性检查，控制给实际
		else
		{
			Seg_Show_Mode = 0;
			if (Temputure_Para_Min_Ctrl > Temputure_Para_Max_Ctrl)
			{
				Error_Set = 1; // 数据参数不合理
			}
			else
			{
				Error_Set = 0; // 数据参数合理
				Temputure_Para_Max = Temputure_Para_Max_Ctrl;
				Temputure_Para_Min = Temputure_Para_Min_Ctrl;
			}
		}
		break;
	case 5:
		if (Seg_Show_Mode)
			Setting_Mode ^= 1;
		break;
	case 6:
		// 当前在参数设置界面
		if (Seg_Show_Mode)
		{
			// 下限设置
			if (Setting_Mode == 0)
			{
				Temputure_Para_Min_Ctrl = (Temputure_Para_Min_Ctrl == 99) ? 0 : Temputure_Para_Min_Ctrl + 1;
			}
			else
			{
				Temputure_Para_Max_Ctrl = (Temputure_Para_Max_Ctrl == 99) ? 0 : Temputure_Para_Max_Ctrl + 1;
			}
		}
		break;
	case 7:
		// 当前在参数设置界面
		if (Seg_Show_Mode)
		{
			// 下限设置
			if (Setting_Mode == 0)
			{
				Temputure_Para_Min_Ctrl = (Temputure_Para_Min_Ctrl == 0) ? 99 : Temputure_Para_Min_Ctrl - 1;
			}
			else
			{
				Temputure_Para_Max_Ctrl = (Temputure_Para_Max_Ctrl == 0) ? 99 : Temputure_Para_Max_Ctrl - 1;
			}
		}
		break;
	}
}

/* 数码管 */
void Seg_Proc()
{
	// 在数据显示界面
	if (Seg_Show_Mode == 0)
	{
		Seg_Buf[0] = 11; // C
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		Seg_Buf[6] = Temputure_Value / 10 % 10;
		Seg_Buf[7] = Temputure_Value % 10;
	}
	// 在参数设置界面
	else
	{
		Seg_Buf[0] = 12; // P
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = Temputure_Para_Max_Ctrl / 10;
		Seg_Buf[4] = Temputure_Para_Max_Ctrl % 10;
		Seg_Buf[5] = 10;
		Seg_Buf[6] = Temputure_Para_Min_Ctrl / 10;
		Seg_Buf[7] = Temputure_Para_Min_Ctrl % 10;
	}
}

/* LED */
void Led_Proc()
{
	ucLed[0] = (Temputure_Value > Temputure_Para_Max);
	ucLed[1] = ((Temputure_Value <= Temputure_Para_Max) && (Temputure_Value >= Temputure_Para_Min));
	ucLed[2] = (Temputure_Value < Temputure_Para_Min);
	ucLed[3] = Error_Set;
	Led_Disp(ucLed);
}

/* AD_DA */
void AD_DA()
{
	if (Temputure_Value > Temputure_Para_Max)
		Da_Write(4 * 51);
	else if ((Temputure_Value <= Temputure_Para_Max) && (Temputure_Value >= Temputure_Para_Min))
		Da_Write(3 * 51);
	else if (Temputure_Value < Temputure_Para_Min)
		Da_Write(2 * 51);
}

/* 温度 */
void Get_Temputure()
{
	Temputure_Value = rd_temperature();
}

/* 定时器 */
void Timer1Init(void) // 1毫秒@12.000MHz
{
	AUXR &= 0xBF; // 定时器时钟12T模式
	TMOD &= 0x0F; // 设置定时器模式
	TL1 = 0x18;	  // 设置定时初值
	TH1 = 0xFC;	  // 设置定时初值
	TF1 = 0;	  // 清除TF1标志
	TR1 = 1;	  // 定时器1开始计时
	ET1 = 1;
	EA = 1;
}

void Timer1Isr() interrupt 3
{
	uwTick++;
	Seg_Pos = (++Seg_Pos) % 8;
	if (Seg_Buf[Seg_Pos] > 20)
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos] - ',', 1);
	else
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos], 0);
}

/* 调度器 */
typedef struct
{
	void (*task_func)(void);   // 任务函数
	unsigned long int rate_ms; // 任务执行周期
	unsigned long int last_ms; // 任务最后一次执行时间
} task_t;

idata task_t Scheduler_Task[] =
	{
		{Led_Proc, 1, 0},
		{Key_Proc, 10, 0},
		{Seg_Proc, 200, 0},
		{AD_DA, 160, 0},
		{Get_Temputure, 300, 0}};

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
		if (now_time >= Scheduler_Task[i].last_ms + Scheduler_Task[i].rate_ms)
		{
			Scheduler_Task[i].last_ms = now_time;
			Scheduler_Task[i].task_func();
		}
	}
}

void Delay750ms() //@12.000MHz
{
	unsigned char i, j, k;

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
	Timer1Init();
	while (1)
	{
		Scheduler_Run();
	}
}