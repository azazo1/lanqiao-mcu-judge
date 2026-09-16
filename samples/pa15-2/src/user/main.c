#include <STC15F2K60S2.H>
#include "key.h"
#include "led.h"
#include "seg.h"
#include "init.h"
#include "iic.h"
#include "filtering.h"
#include "onewire.h"
#include "intrins.h"

/* 变量 */
idata unsigned long int uwTick = 0; // 定时器
// 按键
idata unsigned char Key_Old, Key_Val, Key_Down, Key_Up;
// LED
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
// 数码管
pdata unsigned char Seg_Buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata unsigned char Seg_Pos = 0;

idata unsigned char Temperature_Calibrate_Value = 0; // 温度校准后的值
idata float Temperature_Calibrate_Value_Float = 0;	 // 校准后的值，浮点数，用于判断温度上升情况
idata char Para_Calibrate = 0;						 // 校准值，-9~9
idata char Para_Calibrate_Ctrl = 0;					 // 校准控制，-9~9
idata bit Temperature_Up_Wring = 0;					 // 温度上升超过1度警告
idata bit Temperature_Down_Wring = 0;				 // 温度下降超过1度警告
idata bit Temperature_Wring = 0;					 // 用于报警
idata bit Dac_Work_Mode = 0;						 // DAC控制模式 0 温度 1 手动
idata unsigned char Dac_Digit_Hand = 100;			 // 手动控制电压数字值
idata unsigned char Dac_Digit_Temperature = 0;		 // 温度控制电压数字值

idata unsigned char Seg_Show_Mode = 0; // 显示界面 0 温度界面  1 DAC界面 2 校准值界面
idata bit Relay_Lock = 0;			   // 继电器锁定状态 0 解锁 1 锁定
idata bit Relay_Work_Mode = 0;		   // 继电器工作状态 0 未工作 1 工作

idata bit Long_Press_Flag = 0;			  // 长按检测标志
idata unsigned int Time_1500Ms_Press = 0; // 长按1.5s计时器

idata bit Temperature_Up_Flag = 0;	 // 温度上升
idata bit Temperature_Down_Flag = 0; // 温度下降
idata unsigned int Time_2s_Up = 0;	 // 温度上升2s计时
idata unsigned int Time_2s_Down = 0; // 温度下降2s计时

idata bit Led_Light_Wring = 0;			  // Led闪烁标志位
idata unsigned char Time_200ms_Light = 0; // 200ms闪烁计时
idata unsigned int Time_3s_Wring = 0;	  // 3s闪烁总计时
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
		/* 温度界面 */
		if (Key_Down == 12)
			Seg_Show_Mode = 1;
		// 解锁状态
		if (Relay_Lock == 0)
		{
			if (Key_Down == 16)
				Relay_Work_Mode = 1;
			if (Key_Down == 17)
				Relay_Work_Mode = 0;
		}
		break;
	case 1:
		/* DAC输出界面 */
		if (Key_Down == 12)
		{
			Seg_Show_Mode = 2;
			Para_Calibrate_Ctrl = Para_Calibrate;
		}
		if (Key_Down == 16)
			Dac_Digit_Hand = (Dac_Digit_Hand == 255)
								 ? 255
								 : Dac_Digit_Hand + 5;
		if (Key_Down == 17)
			Dac_Digit_Hand = (Dac_Digit_Hand == 0)
								 ? 0
								 : Dac_Digit_Hand - 5;
		break;
	case 2:
		/* 校准值界面 */
		if (Key_Down == 12)
		{
			Seg_Show_Mode = 0;
			Para_Calibrate = Para_Calibrate_Ctrl;
		}
		if (Key_Down == 16)
			Para_Calibrate_Ctrl = (Para_Calibrate_Ctrl == 9)
									  ? 9
									  : Para_Calibrate_Ctrl + 1;
		if (Key_Down == 17)
			Para_Calibrate_Ctrl = (Para_Calibrate_Ctrl == -9)
									  ? -9
									  : Para_Calibrate_Ctrl - 1;
		break;
	}
	if (Key_Down == 13)
		Long_Press_Flag = 1;
	if (Key_Up == 13)
	{
		// 长按
		if (Time_1500Ms_Press > 1500)
			Relay_Lock ^= 1;
		// 短按
		else
			Dac_Work_Mode ^= 1;
		Long_Press_Flag = 0;
		Time_1500Ms_Press = 0;
	}
}

/* 数码管 */
void Seg_Proc()
{
	unsigned char Temp_Para_Calibrate_Ctrl = -Para_Calibrate_Ctrl;
	switch (Seg_Show_Mode)
	{
	case 0:
		/* 温度界面 */
		Seg_Buf[0] = 11; // C
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		Seg_Buf[6] = Temperature_Calibrate_Value / 10 % 10;
		Seg_Buf[7] = Temperature_Calibrate_Value % 10;
		break;
	case 1:
		/* DAC输出界面 */
		Seg_Buf[0] = 12; // A
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		// 温度控制模式
		if (Dac_Work_Mode == 0)
		{
			Seg_Buf[5] = (Dac_Digit_Temperature / 100 == 0)
							 ? 10
							 : Dac_Digit_Temperature / 100;
			Seg_Buf[6] = ((Dac_Digit_Temperature / 10 % 10 == 0) && (Seg_Buf[5] == 10))
							 ? 10
							 : Dac_Digit_Temperature / 10 % 10;
			Seg_Buf[7] = Dac_Digit_Temperature % 10;
		}
		// 手动控制模式
		else
		{
			Seg_Buf[5] = (Dac_Digit_Hand / 100 == 0)
							 ? 10
							 : Dac_Digit_Hand / 100;
			Seg_Buf[6] = ((Dac_Digit_Hand / 10 % 10 == 0) && (Seg_Buf[5] == 10))
							 ? 10
							 : Dac_Digit_Hand / 10 % 10;
			Seg_Buf[7] = Dac_Digit_Hand % 10;
		}
		break;
	case 2:
		/* 校准值界面 */
		Seg_Buf[0] = 13; // P
		Seg_Buf[1] = 10;
		Seg_Buf[2] = 10;
		Seg_Buf[3] = 10;
		Seg_Buf[4] = 10;
		Seg_Buf[5] = 10;
		if (Para_Calibrate_Ctrl < 0)
		{
			Seg_Buf[6] = 14;
			Seg_Buf[7] = Temp_Para_Calibrate_Ctrl;
		}
		else
		{
			Seg_Buf[6] = 0;
			Seg_Buf[7] = Para_Calibrate_Ctrl;
		}
		break;
	}
}

/* LED */
void Led_Proc()
{
	ucLed[0] = (Dac_Work_Mode == 0);
	ucLed[1] = Temperature_Up_Flag;
	ucLed[2] = Temperature_Down_Flag;
	ucLed[3] = Led_Light_Wring;
	ucLed[7] = (Relay_Lock == 0);
	Relay(Relay_Work_Mode);

	Led_Disp(ucLed);
}

/* AD_DA */
void AD_DA()
{
	// 温度控制模式
	if (Dac_Work_Mode == 0)
	{
		Da_Write(Dac_Digit_Temperature);
	}
	// 手动控制模式
	else
	{
		Da_Write(Dac_Digit_Hand);
	}
}

/* 温度 */
void Get_Temperature()
{
	float temp = rd_temperature();
	temp = Median_Filter(temp) + Para_Calibrate;
	// 温度下降
	if (Temperature_Calibrate_Value_Float > temp)
	{
		Temperature_Down_Flag = 1;
		Temperature_Up_Wring = 0;
		Time_2s_Down = 0;
		// 下降超过1度
		if (Temperature_Calibrate_Value_Float - temp > 1)
		{
			// 第一次超过1度
			if (Temperature_Down_Wring == 0)
				Temperature_Down_Wring = 1;
			// 第二次超过1度
			else
			{
				Temperature_Wring = 1;
				Time_200ms_Light = 0;
				Time_3s_Wring = 0; // 清零计时
			}
		}
		else
		{
			Temperature_Down_Wring = 0;
		}
	}
	// 温度上升
	else if (Temperature_Calibrate_Value_Float < temp)
	{
		Temperature_Up_Flag = 1;
		Time_2s_Up = 0;
		Temperature_Down_Wring = 0;
		// 上升超过1度
		if (temp - Temperature_Calibrate_Value_Float > 1)
		{
			// 第一次超过1度
			if (Temperature_Up_Wring == 0)
				Temperature_Up_Wring = 1;
			// 第二次超过1度
			else
			{
				Temperature_Wring = 1;
				Time_200ms_Light = 0;
				Time_3s_Wring = 0; // 清零计时
			}
		}
		else
		{
			Temperature_Up_Wring = 0;
		}
	}
	Temperature_Calibrate_Value_Float = temp;
	Temperature_Calibrate_Value = Temperature_Calibrate_Value_Float;
	if (Temperature_Calibrate_Value_Float <= 10)
		Dac_Digit_Temperature = 2 * 51;
	else if (Temperature_Calibrate_Value_Float >= 40)
		Dac_Digit_Temperature = 5 * 51;
	else
		Dac_Digit_Temperature = (Temperature_Calibrate_Value_Float / 10.0f + 1) * 51;
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
	if (Long_Press_Flag)
	{
		if (++Time_1500Ms_Press > 1500)
			Time_1500Ms_Press = 1501;
	}
	else
		Time_1500Ms_Press = 0;
	// 温度上升
	if (Temperature_Up_Flag)
	{
		if (++Time_2s_Up == 2000)
		{
			Time_2s_Up = 0;
			Temperature_Up_Flag = 0;
		}
	}
	else
		Time_2s_Up = 0;
	// 温度下降
	if (Temperature_Down_Flag)
	{
		if (++Time_2s_Down == 2000)
		{
			Time_2s_Down = 0;
			Temperature_Down_Flag = 0;
		}
	}
	else
		Time_2s_Down = 0;

	// 温度突变
	if (Temperature_Wring)
	{
		if (++Time_200ms_Light == 200)
		{
			Time_200ms_Light = 0;
			Led_Light_Wring ^= 1;
		}
		if (++Time_3s_Wring == 3000)
		{
			Time_3s_Wring = 0;
			Temperature_Wring = 0;
		}
	}
	else
	{
		Time_200ms_Light = 0;
		Time_3s_Wring = 0;
		Led_Light_Wring = 0;
	}
}

/* 调度器 */
typedef struct
{
	void (*task_func)(void);   // 任务函数
	unsigned long int rate_ms; // 任务周期
	unsigned long int last_ms; // 任务最后一次时间
} task_t;

idata task_t Scheduler_Task[] =
	{
		{Led_Proc, 1, 0},
		{Key_Proc, 10, 0},
		{Seg_Proc, 80, 0},
		{AD_DA, 160, 0},
		{Get_Temperature, 200, 0}};

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