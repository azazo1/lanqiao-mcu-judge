#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"
#include "ds1302.h"
#include "ultrasound.h"

/* 变量 */
idata unsigned long int uwTick = 0; // 定时器
// 按键
idata unsigned char Key_Val, Key_Old, Key_Down, Key_Up;
// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
// 数码管
idata unsigned char Seg_Pos = 0;
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
// 时间
pdata unsigned char ucRtc[3] = {20, 20, 1};

idata unsigned char Seg_Show_Mode = 0;	  // 显示界面 0 数据 1 参数
idata unsigned char Data_Show_Mode = 0;	  // 数据界面 0 时间显示 1 距离显示 2 数据记录
idata unsigned char Para_Show_Mode = 0;	  // 参数页面 0 时间参数 1 距离参数
idata unsigned char Data_Record_Mode = 0; // 数据记录页面 0 最大 1 最小 2 平均
idata bit Work_Mode = 0;				  // 超声波工作模式 0 触发模式 1 定时模式

idata unsigned char Distance_Value = 0;			  // 测距结果
idata unsigned char Distance_Max = 0;			  // 最大距离
idata unsigned char Distance_Min = 0;			  // 最小距离
idata unsigned int Distance_Aver_10x = 0;		  // 10倍平均值
idata unsigned long int Distance_Trigger_Num = 0; // 触发测距次数

idata unsigned char Time_Trigger_Index = 0;				   // 采集时间的下标
idata unsigned char Time_Trigger_Arr[5] = {2, 3, 5, 7, 9}; // 采集时间数组
idata unsigned char Distance_Para = 20;					   // 距离参数
idata unsigned char Time_Trigger_Index_Ctrl = 0;		   // 采集时间的下标控制值
idata unsigned char Distance_Para_Ctrl = 0;				   // 距离参数控制值

idata unsigned char Wring_Data_Num = 0; // 连续测量的值在参数附近（±5）次数
idata bit Time_Trigger_Flag = 0;		// 时间触发标志
idata bit Light_Trigger_Flag = 0;		// 亮暗触发标志
idata unsigned char Light_Voltage_Old;	// 上一时刻的光敏电阻测量值（0~255）
idata bit Light_Flag = 0;				// 当前环境是否亮，0 灭 1 亮
idata bit Acq_Complete = 0;				// 采集完成
/* 按键 */
void Key_Proc()
{
	Key_Val = Key_Read();
	Key_Down = Key_Val & (Key_Val ^ Key_Old);
	Key_Up = ~Key_Val & (Key_Val ^ Key_Old);
	Key_Old = Key_Val;
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 数据界面 */
		// 保证下一个界面是时间参数，真实给控制
		if (Key_Down == 4)
		{
			Seg_Show_Mode = 1;
			Para_Show_Mode = 0;
			Time_Trigger_Index_Ctrl = Time_Trigger_Index;
			Distance_Para_Ctrl = Distance_Para;
		}
		if (Key_Down == 5)
		{
			Data_Show_Mode = (++Data_Show_Mode) % 3;
			Data_Record_Mode = 0;
		}
		if (Key_Down == 8)
		{
			switch (Data_Show_Mode)
			{
			case 1:
				/* 距离显示 */
				Wring_Data_Num = 0;
				Work_Mode ^= 1;
				break;
			case 2:
				/* 数据记录 */
				Data_Record_Mode = (++Data_Record_Mode) % 3;
				break;
			}
		}
		break;
	case 1:
		/* 参数界面 */
		// 保证下一个界面是时间显示，控制给真实
		if (Key_Down == 4)
		{
			Seg_Show_Mode = 0;
			Data_Show_Mode = 0;
			Time_Trigger_Index = Time_Trigger_Index_Ctrl;
			Distance_Para = Distance_Para_Ctrl;
		}
		if (Key_Down == 5)
			Para_Show_Mode = (++Para_Show_Mode) % 2;
		if (Key_Down == 9)
		{
			switch (Para_Show_Mode)
			{
			case 0:
				/* 时间参数 */
				Time_Trigger_Index_Ctrl = (++Time_Trigger_Index_Ctrl) % 5;
				break;
			case 1:
				/* 距离参数 */
				Distance_Para_Ctrl = (Distance_Para_Ctrl == 80)
										 ? 10
										 : Distance_Para_Ctrl + 10;
				break;
			}
		}
		break;
	}
}

/* 数码管 */
void Seg_Proc()
{
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 数据界面 */
		switch (Data_Show_Mode)
		{
		case 0:
			/* 时间显示 */
			Seg_Buf[0] = ucRtc[0] / 10;
			Seg_Buf[1] = ucRtc[0] % 10;
			Seg_Buf[2] = 11; //-
			Seg_Buf[3] = ucRtc[1] / 10;
			Seg_Buf[4] = ucRtc[1] % 10;
			Seg_Buf[5] = 11; //-
			Seg_Buf[6] = ucRtc[2] / 10;
			Seg_Buf[7] = ucRtc[2] % 10;
			break;
		case 1:
			/* 距离显示 */
			Seg_Buf[0] = 12;						 // L
			Seg_Buf[1] = (Work_Mode == 0) ? 13 : 14; // C/F
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			Seg_Buf[4] = 10;
			Seg_Buf[5] = (Distance_Value / 100 == 0)
							 ? 10
							 : Distance_Value / 100;
			Seg_Buf[6] = ((Distance_Value / 10 % 10 == 0) && (Seg_Buf[5] == 10))
							 ? 10
							 : Distance_Value / 10 % 10;
			Seg_Buf[7] = Distance_Value % 10;
			break;
		case 2:
			/* 数据记录 */
			Seg_Buf[0] = 15; // H
			Seg_Buf[2] = 10;
			Seg_Buf[3] = 10;
			switch (Data_Record_Mode)
			{
			case 0:
				/* 最大 */
				Seg_Buf[1] = 16; //-
				Seg_Buf[4] = 10;
				Seg_Buf[5] = (Distance_Max / 100 == 0)
								 ? 10
								 : Distance_Max / 100;
				Seg_Buf[6] = ((Distance_Max / 10 % 10 == 0) && (Seg_Buf[5] == 10))
								 ? 10
								 : Distance_Max / 10 % 10;
				Seg_Buf[7] = Distance_Max % 10;
				break;
			case 1:
				/* 最小 */
				Seg_Buf[1] = 17; //-
				Seg_Buf[4] = 10;
				Seg_Buf[5] = (Distance_Min / 100 == 0)
								 ? 10
								 : Distance_Min / 100;
				Seg_Buf[6] = ((Distance_Min / 10 % 10 == 0) && (Seg_Buf[5] == 10))
								 ? 10
								 : Distance_Min / 10 % 10;
				Seg_Buf[7] = Distance_Min % 10;
				break;
			case 2:
				/* 平均 */
				Seg_Buf[1] = 11; //-
				Seg_Buf[4] = (Distance_Aver_10x / 1000 == 0)
								 ? 10
								 : Distance_Aver_10x / 1000;
				Seg_Buf[5] = ((Distance_Aver_10x / 100 % 10 == 0) && (Seg_Buf[4] == 10))
								 ? 10
								 : Distance_Aver_10x / 100 % 10;
				Seg_Buf[6] = Distance_Aver_10x / 10 % 10 + ',';
				Seg_Buf[7] = Distance_Aver_10x % 10;
				break;
			}
			break;
		}
		break;
	case 1:
		/* 参数界面 */
		Seg_Buf[0] = 18; // P
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		switch (Para_Show_Mode)
		{
		case 0:
			/* 时间参数 */
			Seg_Buf[1] = 1;
			Seg_Buf[6] = 0;
			Seg_Buf[7] = Time_Trigger_Arr[Time_Trigger_Index_Ctrl];
			break;
		case 1:
			/* 距离参数 */
			Seg_Buf[1] = 2;
			Seg_Buf[6] = Distance_Para_Ctrl / 10;
			Seg_Buf[7] = Distance_Para_Ctrl % 10;
			break;
		}
		break;
	}
}

/* LED */
void Led_Proc()
{
	ucLed[0] = ((Seg_Show_Mode == 0) && (Data_Show_Mode == 0));
	ucLed[1] = ((Seg_Show_Mode == 0) && (Data_Show_Mode == 1));
	ucLed[2] = ((Seg_Show_Mode == 0) && (Data_Show_Mode == 2));
	ucLed[3] = (Work_Mode == 0);

	ucLed[5] = Light_Flag;
	Led_Disp(ucLed);
}

/* 超声波 */
void Get_Distance()
{
	// 如果在触发模式下
	if (Work_Mode == 0)
	{
		Wring_Data_Num = 0;
		// 没有采集，并且亮暗触发采集
		if ((Acq_Complete == 0) && Light_Trigger_Flag)
		{
			Distance_Value = Ut_Wave_Data();
			Acq_Complete = 1;
			if (Distance_Value != 0)
			{
				if (Distance_Value > Distance_Max)
					Distance_Max = Distance_Value;
				// 当最小值为0的时候，直接把现在的数据写入
				if (Distance_Min == 0)
					Distance_Min = Distance_Value;
				else
				{
					if (Distance_Value < Distance_Min)
						Distance_Min = Distance_Value;
				}
				Distance_Trigger_Num++;
				Distance_Aver_10x = (float)(Distance_Value * 10 + Distance_Aver_10x * (Distance_Trigger_Num - 1)) / Distance_Trigger_Num;
			}
		}
	}
	// 如果在时间模式下
	else
	{
		// 没有采集，并且时间触发采集
		if ((Acq_Complete == 0) && Time_Trigger_Flag)
		{
			Distance_Value = Ut_Wave_Data();
			Acq_Complete = 1;
			// 在参数±5范围内
			if ((Distance_Value <= Distance_Para + 5) || (Distance_Value >= Distance_Para - 5))
			{
				if (++Wring_Data_Num >= 3)
					Wring_Data_Num = 4;
			}
			else
				Wring_Data_Num = 0;
			if (Distance_Value != 0)
			{
				if (Distance_Value > Distance_Max)
					Distance_Max = Distance_Value;
				// 当最小值为0的时候，直接把现在的数据写入
				if (Distance_Min == 0)
					Distance_Min = Distance_Value;
				else
				{
					if (Distance_Value < Distance_Min)
						Distance_Min = Distance_Value;
				}
				Distance_Trigger_Num++;
				Distance_Aver_10x = (float)(Distance_Value * 10 + Distance_Aver_10x * (Distance_Trigger_Num - 1)) / Distance_Trigger_Num;
			}
		}
	}
}

/* 时间 */
void Get_Time()
{
	Read_Rtc(ucRtc);
	// 时间触发
	if (ucRtc[2] % Time_Trigger_Arr[Time_Trigger_Index] == 0)
	{
		Time_Trigger_Flag = 1;
		Acq_Complete = 0;
	}
	else
	{
		Time_Trigger_Flag = 0;
	}
}

/* AD_DA */
void AD_DA()
{
	unsigned char temp;
	float temp_da;
	temp = Ad_Read(0x41);
	// 亮->暗触发
	if ((Light_Voltage_Old > 100) && (temp < 100))
	{
		Light_Trigger_Flag = 1;
		Acq_Complete = 0;
	}
	else
	{
		Light_Trigger_Flag = 0;
	}
	Light_Flag = (temp > 100); // 亮状态
	Light_Voltage_Old = temp;

	if (Distance_Value <= 10)
		Da_Write(1 * 51);
	else if (Distance_Value >= 80)
		Da_Write(5 * 51);
	else
	{
		temp_da = 4 * (float)(Distance_Value - 10) / 70 + 1;
		Da_Write(temp_da * 51);
	}
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
		{Seg_Proc, 180, 0},
		{AD_DA, 160, 0},
		{Get_Distance, 100, 0},
		{Get_Time, 150, 0}};

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

void main()
{
	System_Init();
	Set_Rtc(ucRtc);
	Scheduler_Init();
	Timer1_Init();
	while (1)
	{
		Scheduler_Run();
	}
}