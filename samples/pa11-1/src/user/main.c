#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"

/* 变量 */
idata unsigned long int uwTick = 0; // 系统计时
// 按键
idata unsigned char Key_Val, Key_Old, Key_Down, Key_Up;
// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
// 数码管
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata unsigned char Seg_Pos = 0;

idata unsigned char Seg_Show_Mode = 0;	  // 数码管显示界面 0 数据 1 参数 2 计数
idata unsigned int AD_100x = 0;			  // AD采集的100倍
idata unsigned int AD_Old_100x = 0;		  // 上一次AD采集的100倍
idata unsigned char AD_Para_10x = 0;	  // 参考电压的10倍
idata unsigned char AD_Para_Ctrl_10x = 0; // 参考电压控制值的10倍
idata unsigned long int Count_Down = 0;	  // 下降沿计数
idata unsigned char EEPROM_Lock = 5;

idata bit Low_Flag = 0;				 // 低电平标志
idata unsigned char Key_Error = 0;	 // 无效按键记录次数
idata unsigned int Time_Tick_5s = 0; // 5s计数
/* 按键 */
void Key_Proc()
{
	Key_Val = Key_Read();
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val;
	switch (Key_Down)
	{
	case 12:
		Key_Error = 0;
		// 如果所处界面为数据界面，参数值给控制值
		if (Seg_Show_Mode == 0)
			AD_Para_Ctrl_10x = AD_Para_10x;
		// 如果所处界面为参数界面，控制值给参数值，并存储
		else if (Seg_Show_Mode == 1)
		{
			AD_Para_10x = AD_Para_Ctrl_10x;
			EEPROM_Write(&AD_Para_10x, 0, 1);
			EEPROM_Write(&EEPROM_Lock, 8, 1);
		}
		Seg_Show_Mode = (++Seg_Show_Mode) % 3;

		break;
	case 13:
		if (Seg_Show_Mode == 2)
		{
			Key_Error = 0;
			Count_Down = 0;
		}
		else
		{
			if (Key_Error <= 3)
				Key_Error++;
		}
		break;
	case 16:
		if (Seg_Show_Mode == 1)
		{
			Key_Error = 0;
			AD_Para_Ctrl_10x = (AD_Para_Ctrl_10x == 50) ? 0
														: AD_Para_Ctrl_10x + 5;
		}
		else
		{
			if (Key_Error <= 3)
				Key_Error++;
		}
		break;
	case 17:

		if (Seg_Show_Mode == 1)
		{
			Key_Error = 0;
			AD_Para_Ctrl_10x = (AD_Para_Ctrl_10x == 0) ? 50
													   : AD_Para_Ctrl_10x - 5;
		}
		else
		{
			if (Key_Error <= 3)
				Key_Error++;
		}
		break;

	default:
		if (Key_Down != 0)
			if (Key_Error <= 3)
				Key_Error++;
		break;
	}
}

/* 数码管 */
void Seg_Proc()
{
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 数据显示 */
		Seg_Buf[0] = 11; // U
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = AD_100x / 100 + ',';
		Seg_Buf[6] = AD_100x / 10 % 10;
		Seg_Buf[7] = AD_100x % 10;
		break;
	case 1:
		/* 参数设置 */
		Seg_Buf[0] = 12; // P
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = AD_Para_Ctrl_10x / 10 + ',';
		Seg_Buf[6] = AD_Para_Ctrl_10x % 10;
		Seg_Buf[7] = 0;
		break;
	case 2:
		/* 计数界面 */
		Seg_Buf[0] = 13; // n
		Seg_Buf[1] = (Count_Down / 1000000 % 10 == 0) ? 10 : Count_Down / 1000000 % 10;
		Seg_Buf[2] = ((Count_Down / 100000 % 10 == 0) && (Seg_Buf[1] == 10)) ? 10 : Count_Down / 100000 % 10;
		Seg_Buf[3] = ((Count_Down / 10000 % 10 == 0) && (Seg_Buf[2] == 10)) ? 10 : Count_Down / 10000 % 10;
		Seg_Buf[4] = ((Count_Down / 1000 % 10 == 0) && (Seg_Buf[3] == 10)) ? 10 : Count_Down / 1000 % 10;
		Seg_Buf[5] = ((Count_Down / 100 % 10 == 0) && (Seg_Buf[4] == 10)) ? 10 : Count_Down / 100 % 10;
		Seg_Buf[6] = ((Count_Down / 10 % 10 == 0) && (Seg_Buf[5] == 10)) ? 10 : Count_Down / 10 % 10;
		Seg_Buf[7] = ((Count_Down % 10 == 0) && (Seg_Buf[6] == 10)) ? 10 : Count_Down % 10;
		break;
	}
}

/* LED */
void Led_Proc()
{
	ucLed[0] = (Time_Tick_5s >= 5000);
	ucLed[1] = (Count_Down % 2);
	ucLed[2] = (Key_Error >= 3);
	Led_Disp(ucLed);
}

/* AD */
void Get_AD()
{
	AD_100x = Ad_Read(0x03) * 100 / 51;
	if ((AD_100x <= AD_Para_10x * 10) && (AD_Old_100x >= AD_Para_10x * 10))
		Count_Down++;
	AD_Old_100x = AD_100x;
	if (AD_100x <= AD_Para_10x * 10)
		Low_Flag = 1;
	else
		Low_Flag = 0;
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
	// 如果处于低电平，开启计数
	if (Low_Flag)
	{
		if (++Time_Tick_5s >= 5000)
			Time_Tick_5s = 5001;
	}
	// 如果不处于低电平
	else
	{
		Time_Tick_5s = 0;
	}
}

/* 调度器 */
typedef struct
{
	void (*task_func)(void);   // 任务函数
	unsigned long int rate_ms; // 任务执行周期
	unsigned long int last_ms; // 最后一次任务时间
} task_t;

idata task_t Scheduler_Task[] =
	{
		{Led_Proc, 1, 0},
		{Key_Proc, 10, 0},
		{Seg_Proc, 300, 0},
		{Get_AD, 100, 0}};

idata unsigned char task_num; // 任务数目

void Scheduler_Init()
{
	task_num = sizeof(Scheduler_Task) / sizeof(task_t);
}

void Scheduler_Run()
{
	unsigned char i;
	for (i = 0; i < task_num; i++)
	{
		unsigned long int now = uwTick;
		if (now >= Scheduler_Task[i].last_ms + Scheduler_Task[i].rate_ms)
		{
			Scheduler_Task[i].last_ms = now;
			Scheduler_Task[i].task_func();
		}
	}
}
void main()
{
	unsigned char EEPROM_Temp;
	System_Init();
	Scheduler_Init();
	EEPROM_Read(&EEPROM_Temp, 8, 1);
	// 上次写入有效
	if (EEPROM_Temp == EEPROM_Lock)
	{
		EEPROM_Read(&AD_Para_10x, 0, 1);
	}
	Timer1Init();
	while (1)
	{
		Scheduler_Run();
	}
}