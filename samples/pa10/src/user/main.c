#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"

/* 变量 */
// 定时器
idata unsigned long int uwTick = 0; // 系统定时器
idata unsigned int Time_1s = 0;		// 定时1s

// 按键
idata unsigned char Key_Val, Key_Old, Key_Down, Key_Up;

// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};

// 数码管
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata unsigned char Seg_Pos = 0;

// AD/DA
idata unsigned int AD_in_100x; // AD输入的100倍
idata unsigned char DA_Out;	   // DA输出

// 频率
idata unsigned int Freq;

idata unsigned char Seg_Show_Mode = 0; // 0 电压 1 频率界面
idata bit DA_Out_Mode = 0;			   // 0 固定2V 1 跟随AD
idata bit Led_Mode = 1;				   // 是否开启功能 1 启用 0 关闭
idata bit Seg_Mode = 1;				   // 是否开启功能 1 启用 0 关闭
/* 按键处理 */
void Key_Proc()
{
	Key_Val = Key_Read();
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val;
	switch (Key_Down)
	{
	case 4:
		Seg_Show_Mode = (++Seg_Show_Mode) % 2;
		break;
	case 5:
		DA_Out_Mode ^= 1;
		break;
	case 6:
		Led_Mode ^= 1;
		break;
	case 7:
		Seg_Mode ^= 1;
		break;
	}
}

/* 数码管 */
void Seg_Proc()
{
	// 如果开启数码管
	if (Seg_Mode)
	{
		switch (Seg_Show_Mode)
		{
		case 0:
			/* 电压 */
			Seg_Buf[0] = 11; // U
			Seg_Buf[1] = 10;
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = AD_in_100x / 100 + ',';
			Seg_Buf[6] = AD_in_100x / 10 % 10;
			Seg_Buf[7] = AD_in_100x % 10;
			break;

		case 1:
			/* 频率 */
			Seg_Buf[0] = 12; // F
			Seg_Buf[1] = 10;
			Seg_Buf[2] = (Freq / 100000 % 10 == 0) ? 10 : Freq / 100000 % 10;
			Seg_Buf[3] = ((Freq / 10000 % 10 == 0) && (Seg_Buf[2] == 10)) ? 10 : Freq / 10000 % 10;
			Seg_Buf[4] = ((Freq / 1000 % 10 == 0) && (Seg_Buf[3] == 10)) ? 10 : Freq / 1000 % 10;
			Seg_Buf[5] = ((Freq / 100 % 10 == 0) && (Seg_Buf[4] == 10)) ? 10 : Freq / 100 % 10;
			Seg_Buf[6] = ((Freq / 10 % 10 == 0) && (Seg_Buf[5] == 10)) ? 10 : Freq / 10 % 10;
			Seg_Buf[7] = ((Freq % 10 == 0) && (Seg_Buf[6] == 10)) ? 10 : Freq % 10;
			break;
		}
	}
	// 如果关闭数码管
	else
	{
		Seg_Buf[0] = 10;
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		Seg_Buf[6] = 10;
		Seg_Buf[7] = 10;
	}
}

/* LED */
void Led_Proc()
{
	// LED启用
	if (Led_Mode)
	{
		ucLed[0] = (Seg_Show_Mode == 0);
		ucLed[1] = (Seg_Show_Mode == 1);
		ucLed[2] = (((AD_in_100x >= 150) && (AD_in_100x < 250)) ||
					(AD_in_100x >= 350));
		ucLed[3] = (((Freq >= 1000) && (Freq < 5000)) ||
					(Freq >= 10000));
		ucLed[4] = DA_Out_Mode;
	}
	// LED禁用
	else
	{
		ucLed[0] = 0;
		ucLed[1] = 0;
		ucLed[2] = 0;
		ucLed[3] = 0;
		ucLed[4] = 0;
		ucLed[5] = 0;
		ucLed[6] = 0;
		ucLed[7] = 0;
	}
	Led_Disp(ucLed);
}

/* AD_DA */
void AD_DA()
{
	unsigned char temp = Ad_Read(0x43);
	AD_in_100x = temp * 100 / 51;
	// 如果DA跟随AD
	if (DA_Out_Mode)
		Da_Write(temp);
	// 如果DA固定输出
	else
		Da_Write(2 * 51);
}

/* 定时器 */
void Timer0Init(void)
{
	AUXR &= 0x7F; // 定时器时钟12T模式
	TMOD &= 0xF0; // 设置定时器模式
	TMOD |= 0x05; // 设置为计数模式
	TL0 = 0;	  // 设置定时初值
	TH0 = 0;	  // 设置定时初值
	TF0 = 0;	  // 清除TF0标志
	TR0 = 1;	  // 定时器0开始计时
}

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
	if (++Time_1s == 1000)
	{
		Time_1s = 0;
		Freq = (TH0 << 8) | TL0;
		TH0 = TL0 = 0;
	}
	if (Seg_Buf[Seg_Pos] > 20)
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos] - ',', 1);
	else
		Seg_Disp(Seg_Pos, Seg_Buf[Seg_Pos], 0);
}
// 调度器
typedef struct
{
	void (*task_func)(void);   // 任务函数
	unsigned long int rate_ms; // 任务周期
	unsigned long int last_ms; // 最后一次执行的时间
} task_t;

idata task_t Scheduler_Task[] = {
	{Led_Proc, 1, 0},
	{Key_Proc, 10, 0},
	{Seg_Proc, 300, 0},
	{AD_DA, 160, 0}};

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

void main()
{
	System_Init();
	Timer0Init();
	Scheduler_Init();
	Timer1Init();
	while (1)
	{
		Scheduler_Run();
	}
}