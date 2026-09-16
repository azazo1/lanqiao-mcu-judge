#include <STC15F2K60S2.H>
#include "Init.h"
#include "Seg.h"
#include "Key.h"
#include "Led.h"
#include "iic.h"
#include "ds1302.h"
#include "ds18b20.h"

typedef unsigned char u8;
typedef unsigned int u16;
typedef unsigned long int u32;

/* 界面参数 */  
idata bit MainMode;			  //主界面 0-数据界面 1-参数界面
idata u8 SegMode; 		  	//数据界面分界面 0-时间 1-温度 2-亮暗状态
idata u8 SetMode;         //参数界面分界面 0-时间参数 1-温度参数 2-指示灯参数
/* AD */
idata u16 RD1_100x;       //光敏电阻电压放大100倍
idata bit DarkFlag;       //环境为暗检测标志位 0-亮 1-暗
/* 温度 */
idata u16 Tem_10x;        //温度读取放大10倍
idata u8 TemSet = 25;     //温度参数，默认值25，范围00~99
idata u8 TemSetDis = 25;  //温度参数在修改过程中的值
/* 时间 */
pdata u8 Rtc[3] = {0x16,0x59,0x50};
idata u8 HourSet = 17;    //小时参数，默认值17，范围00~23	
idata u8 HourSetDis = 17; //小时参数在修改过程中的值
/* 按键 */
idata u8 KeyVal,KeyDown,KeyUp,KeyOld;
/* LED */
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata u8 LedSet = 4;      //指示灯参数，默认值4，范围4~8
idata u8 LedSetDis = 4;   //指示灯参数修改过程中的值
idata bit L1Light;        //指示灯L1点亮标志位 0灭 1亮
idata bit L2Light;        //指示灯L2点亮标志位 0灭 1亮
idata bit L3Light;        //指示灯L3点亮标志位 0灭 1亮
idata u16 Time_3s;        //暗环境计时变量
idata u16 Time_3000ms;    //亮环境计时变量
/* 数码管 */
idata u8 SegPos;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
/* 调度器 */
idata u32 systick;

void KeyProc()
{
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyDown = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	switch(KeyDown)
	{
		case 4:
			if(!MainMode)
			{
				TemSetDis = TemSet;
				HourSetDis = HourSet;
				LedSetDis = LedSet;
				MainMode = 1;
				SetMode = 0;
			}
			else
			{
				TemSet = TemSetDis;
				HourSet = HourSetDis;
				LedSet = LedSetDis;
				MainMode = 0;
				SegMode = 0;
			}
		break;
		
		case 5:
			if(!MainMode)//数据界面
			{
				SegMode++;
				if(SegMode == 3)
					SegMode = 0;
			}
			else//参数界面
			{
				SetMode++;
				if(SetMode == 3)
					SetMode = 0;
			}
		break;
			
		case 8://减按键
			if(MainMode)
			{
				if(SetMode == 0)
				{
					if(--HourSetDis == 255)
						HourSetDis = 0;
				}
				else if(SetMode == 1)
				{
					if(--TemSetDis == 255)
						TemSetDis = 0;
				}
				else
				{
					if(--LedSetDis == 3)
						LedSetDis = 4;
				}
			}
		break;
			
		case 9://加按键
			if(MainMode)
			{
				if(SetMode == 0)
				{
					if(++HourSetDis == 24)
						HourSetDis = 23;
				}
				else if(SetMode == 1)
				{
					if(++TemSetDis == 100)
						TemSetDis = 99;
				}
				else
				{
					if(++LedSetDis == 9)
						LedSetDis = 8;
				}
			}
		break;
	}
}

void SegProc()
{
	unsigned char i;
	if(!MainMode)//数据界面
	{
		switch(SegMode)
		{
			case 0:
				SegPoint[2] = 0;
				SegBuf[2] = SegBuf[5] = 11;
				for(i = 0; i < 3; i++)
				{
					SegBuf[3*i] = Rtc[i] / 16;
					SegBuf[3*i+1] = Rtc[i] % 16;
				}
			break;
				
			case 1:
				SegBuf[0] = 12;
				SegBuf[1] = SegBuf[2] = SegBuf[3] = SegBuf[4] = 10;
				SegBuf[5] = Tem_10x / 100;
				SegBuf[6] = Tem_10x / 10 % 10;
				SegBuf[7] = Tem_10x % 10;
				SegPoint[6] = 1;
			break;
			
			case 2:
				SegBuf[0] = 13;
				SegBuf[1] = 10;
				SegBuf[2] = RD1_100x / 100;
				SegBuf[3] = RD1_100x / 10 % 10;
				SegBuf[4] = RD1_100x % 10;
				SegBuf[5] = 10;
				SegBuf[6] = 10;
				SegBuf[7] = DarkFlag;
				SegPoint[2] = 1;
				SegPoint[6] = 0;
			break;
		}
	}
	else//参数界面
	{
		switch(SetMode)
		{
			case 0:
				SegPoint[2] = SegPoint[6] = 0;
				SegBuf[0] = 13;
				SegBuf[1] = 4;
				SegBuf[2] = 10;
				SegBuf[3] = 10;
				SegBuf[4] = 10;
				SegBuf[5] = 10;
				SegBuf[6] = (HourSetDis / 10) ? HourSetDis / 10 : 10;
				SegBuf[7] = HourSetDis % 10;
			break;
			
			case 1:
				SegBuf[0] = 13;
				SegBuf[1] = 5;
				SegBuf[6] = (TemSetDis / 10) ? TemSetDis / 10 : 10;
				SegBuf[7] = TemSetDis % 10;
			break;
			
			case 2:
				SegBuf[0] = 13;
				SegBuf[1] = 6;
				SegBuf[6] = 10;
				SegBuf[7] = LedSetDis;
			break;
		}
	}
}

void LedProc()
{
	unsigned char i;
	ucLed[0] = L1Light;
	ucLed[1] = L2Light;
	ucLed[2] = L3Light;
	if(DarkFlag)
	{
		for(i = 3; i < 8; i++)
			ucLed[i] = (i == LedSet-1);
	}
	else
		for(i = 3; i < 8; i++)
			ucLed[i] = 0;
	LedDisp(ucLed);
}

void DS18B20Proc()
{
	Tem_10x = TemRead() * 10;
	L2Light = (Tem_10x < TemSet * 10);
}

void DS1302Proc()
{
	unsigned char Hour;
	GetRtc(Rtc);
	Hour = (Rtc[0] / 16) * 10 + Rtc[0] % 16;
	L1Light = (HourSet <= Hour && Hour < 32);
}

void ADProc()
{
	RD1_100x = AverageFilter() / 51.0 * 100;
	DarkFlag = (RD1_100x < 100) ? 1 : 0;
	DarkFlag ? (Time_3000ms = 0) : (Time_3s = 0);
}

void Timer0_Init(void)		//1毫秒@12.000MHz
{
	AUXR &= 0x7F;			//定时器时钟12T模式
	TMOD &= 0xF0;			//设置定时器模式
	TL0 = 0x18;				//设置定时初始值
	TH0 = 0xFC;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
	ET0 = 1;				//使能定时器0中断
	EA = 1;
}

void Timer0_Isr(void) interrupt 1
{
	systick++;
	if(++SegPos == 8)
		SegPos = 0;
	
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	
	if(DarkFlag)
	{
		if(++Time_3s >= 3001)
		{
			Time_3s = 3001;
			L3Light = 1;
		}
	}
	else
	{
		if(++Time_3000ms >= 3001)
		{
			Time_3000ms = 3001;
			L3Light = 0;
		}
	}
}

typedef struct {
	void (*func)(void);
	u32 rate_ms;
	u32 last_ms;
}task;

idata task scheduler_task[] = {
	{LedProc,1,0},
	{KeyProc,10,0},
	{SegProc,100,0},
	{DS1302Proc,200,0},
	{ADProc, 200,0},
	{DS18B20Proc,200,0}
};

idata u8 task_num;

void SchedulerInit()
{
	task_num = sizeof(scheduler_task) / sizeof(task);
}

void SchedulerRun()
{
	u8 i;
	for(i = 0; i < task_num; i++)
	{
		u32 nownum = systick;
		if(nownum >= scheduler_task[i].rate_ms + scheduler_task[i].last_ms)
		{
			scheduler_task[i].last_ms = nownum;
			scheduler_task[i].func();
		}
	}
}

void main()
{
	SystemInit();
	Timer0_Init();
	SetRtc(Rtc);
	SchedulerInit();
	while(1)
	{
		SchedulerRun();
	}
}