#include <Init.h>
#include <led.h>
#include <key.h>
#include <seg.h>
#include <wave.h>
#include <iic.h>

typedef unsigned char u8;
typedef unsigned int u16;
typedef unsigned long int u32;
/*按键*/
idata u8 KeyVal,KeyDown,KeyUp,KeyOld;
idata bit KeyPressFlag;        //按键长按标志位
idata u16 Time_1000ms;         //长按1s
/*数码管*/
idata u8 SegPos;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
idata u8 SegMode;        		   //主页面 0-频率页面 1-湿度页面 2-测距页面 3-参数页面
idata u8 SetMode;        			 //参数页面 0-频率参数 1-湿度参数 2-距离参数
idata bit distance_mode; 			 //距离控制标志位 0-cm 1-m
idata bit freq_mode;     			 //频率控制标志位 0-hz 1-khz
/*频率*/
idata u16 freq, Time_1s; 	 	 	 //频率
idata float freq_khz;
idata float freq_set = 9; 	 	 //频率参数 初值9khz 范围1.0~12.0
idata bit freq_flag;           //频率数据大于频率参数时为1
/*湿度*/
idata u8 humidity;      	 	 	 //湿度
idata u8 humidity_set = 40;		 //湿度参数 初值40 范围10~60
idata bit humidity_flag;       //湿度数据大于湿度参数时为1
/*超声波*/
idata u8 distance_cm;     		 //超声波测距 cm
idata u8 distance_m_10x = 6;   //超声波参数，初值0.6m 范围0.1~1.2m
idata bit distance_flag;       //超声波数据大于超声波参数时为1
/*指示灯*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata bit LedFlash;            //指示灯闪烁标志位
idata u8 Time_100ms;           //100ms闪烁1次
idata u8 RelayCount;           //继电器开关次数
idata u8 pwm_level;            //PWM输出占空比
idata u8 pwm_count;						 //PWM计时

idata u32 systick;

void KeyProc()
{
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	if(SegMode == 1)//湿度页面下长按清空继电器计数值
	{
		if(KeyDown == 7)
			KeyPressFlag = 1;
		if(KeyUp == 7)
		{
			KeyPressFlag = 0;
			if(Time_1000ms >= 1000)
			{
				RelayCount = 0;
			}
		}
	}

	switch(KeyDown)
	{
		case 4:
			if(++SegMode == 4)
				SegMode = 0;
			if(SegMode == 0)
				SetMode = 0;
		break;
			
		case 5:
			if(SegMode == 3)
				if(++SetMode == 3)
					SetMode = 0;
		break;
		
		case 6:
			if(SegMode == 2)
				distance_mode = !distance_mode;
			else if(SegMode == 3)//+
			{
				if(!SetMode)//频率参数每次加0.5
				{
					freq_set += 0.5;
					if(freq_set == 12.5)
						freq_set = 1.0;
				}
				else if(SetMode == 1)//湿度参数每次加10
				{
					humidity_set += 10;
					if(humidity_set == 70)
						humidity_set = 10;
				}
				else//距离参数每次加0.1 也就是加1
				{
					if(++distance_m_10x == 13)
						distance_m_10x = 1;
				}
			}
		break;
		
		case 7:
			if(!SegMode)
				freq_mode = !freq_mode;
			else if(SegMode == 3)//-
			{
				if(!SetMode)//频率参数每次-0.5
				{
					freq_set -= 0.5;
					if(freq_set == 0.5)
						freq_set = 12.0;
				}
				else if(SetMode == 1)//湿度参数每次-10
				{
					humidity_set -= 10;
					if(humidity_set == 0)
						humidity_set = 60;
				}
				else//距离参数每次-0.1 也就是-1
				{
					if(--distance_m_10x == 0)
						distance_m_10x = 12;
				}
			}
		break;
	}
}

void SegProc()
{
	switch(SegMode)
	{	
		case 0://频率页面
			SegBuf[0] = 11;//F
			SegBuf[1] = 10;
			SegBuf[2] = 10;
			SegPoint[6] = freq_mode;
			if(!freq_mode)
			{
					SegBuf[3] = (freq/10000%10)?freq/10000%10:10;
					SegBuf[4] = (freq/1000%10==0&&SegBuf[3]==10)?10:freq/1000%10;
					SegBuf[5] = (freq/100%10==0&&SegBuf[4]==10)?10:freq/100%10;
					SegBuf[6] = (freq/10%10==0&&SegBuf[5]==10)?10:freq/10%10;
					SegBuf[7] = freq%10;
			}
			else
			{
				SegBuf[3] = 10;
				SegBuf[4] = 10;
				SegBuf[5] = (u8)freq_khz / 10 % 10;
				if(!SegBuf[5]) SegBuf[5] = 10;
				SegBuf[6] = (u8)freq_khz % 10;
				SegBuf[7] = (u8)(freq_khz * 10) % 10;
			}
		break;
			
		case 1://湿度页面
			SegBuf[0] = 12;//H
			SegBuf[3] = 10;
			SegBuf[4] = 10;
			SegBuf[5] = 10;
			SegBuf[6] = humidity / 10;
			SegBuf[7] = humidity % 10;
			SegPoint[6] = 0;
		break;
		
		case 2://测距页面
			SegBuf[0] = 13;//A
			if(!distance_mode)//cm
			{
				if(distance_cm < 10)
				{
					SegBuf[5] = 10;
					SegBuf[6] = 10;
					SegBuf[7] = distance_cm % 10;
				}
				else
				{
					SegBuf[5] = distance_cm / 100 ? distance_cm / 100 : 10;
					SegBuf[6] = distance_cm / 10 % 10;
					SegBuf[7] = distance_cm % 10;
				}
				SegPoint[5] = 0;
			}
			else//m
			{
				SegBuf[5] = distance_cm / 100;
				SegBuf[6] = distance_cm / 10 % 10;
				SegBuf[7] = distance_cm % 10;
				SegPoint[5] = 1;
			}
		break;
			
		case 3://参数页面
			SegBuf[0] = 14;//P
			SegBuf[1] = SetMode + 1;
			SegBuf[2] = 10;
			SegBuf[3] = 10;
			SegBuf[4] = 10;
			switch(SetMode)
			{
				case 0://频率参数
					SegBuf[5] = (u8)freq_set / 10 ? freq_set / 10 : 10;
					SegBuf[6] = (u8)freq_set % 10;
					SegBuf[7] = (u8)(freq_set * 10) % 10;
					SegPoint[6] = 1;
				break;
				
				case 1://湿度参数
					SegBuf[5] = 10;
					SegBuf[6] = humidity_set / 10;
					SegBuf[7] = humidity_set % 10;
					SegPoint[6] = 0;
				break;
				
				case 2://距离参数
					SegBuf[6] = distance_m_10x / 10;
					SegBuf[7] = distance_m_10x % 10;
					SegPoint[6] = 1;
				break;
			}
		break;
	}
}

void WaveProc()
{
	distance_cm = GetWave() + 2;
	distance_flag = (distance_cm > distance_m_10x*10);
}

void LedProc()
{
	u8 i;
	idata bit relay_has_count;//继电器计数重复标志位
	/*继电器*/
	if(!relay_has_count && distance_flag)
	{
		relay_has_count = 1;
		RelayCount++;
	}
	else if(relay_has_count && !distance_flag)
	{
		relay_has_count = 0;
		RelayCount++;
	}
	Relay(distance_flag);
	/*Led*/
	if(SegMode != 3)
	{
		for(i = 0; i < 3; i++)
			ucLed[i] = (i == SegMode);
	}
	else
	{
		for(i = 0; i < 3; i++)
			ucLed[i] = (i == SetMode && LedFlash);
	}
	ucLed[3] = freq_flag;
	ucLed[4] = humidity_flag;
	ucLed[5] = distance_flag;
	LedDisp(ucLed);
	/*MOTOR*/
	pwm_level = freq_flag ? 8 : 2;
}

void IICProc()
{
	float dat;
	/*AD*/
	float RB2 = AdRead() / 51.0;
	if(RB2 >= 5.0)
		humidity = 100;
	else
		humidity = 20 * RB2;
	/*DA*/
	if(humidity <= humidity_set)
		dat = 1.0;
	else if(humidity >= 80)
		dat = 5.0;
	else
		dat = 4.0/(80-humidity_set)*humidity+1-4.0*humidity_set/(80-humidity_set);
	DaWrite(dat * 51);
	humidity_flag = (humidity > humidity_set);
	/*EEPROM*/
	EepromWrite(&RelayCount,0,1);
}

void Timer0_Init(void)		//0微秒@12.000MHz
{
	TMOD &= 0xF0;			//设置定时器模式
	TMOD |= 0x05;			//设置定时器模式
	TL0 = 0;				//设置定时初始值
	TH0 = 0;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
}

void Timer1_Init(void)		//1毫秒@12.000MHz
{
	AUXR &= 0xBF;			//定时器时钟12T模式
	TMOD &= 0x0F;			//设置定时器模式
	TL1 = 0x18;				//设置定时初始值
	TH1 = 0xFC;				//设置定时初始值
	TF1 = 0;				//清除TF1标志
	TR1 = 1;				//定时器1开始计时
	ET1 = 1;				//使能定时器1中断
	EA = 1;
}

void Timer2_Init(void)		//100微秒@12.000MHz
{
	AUXR &= 0xFB;			//定时器时钟12T模式
	T2L = 0x9C;				//设置定时初始值
	T2H = 0xFF;				//设置定时初始值
	AUXR |= 0x10;			//定时器2开始计时
	IE2 |= 0x04;			//使能定时器2中断
}

void Timer1_Isr(void) interrupt 3
{
	systick++;
	if(++SegPos == 8)
		SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	//NE555
	if(++Time_1s == 1000)
	{
		Time_1s = 0;
		freq = (TH0 << 8) | TL0;
		freq_khz = freq / 1000.0;
		TH0 = TL0 = 0;
		freq_flag = (freq_khz > freq_set);
	}
	//指示灯闪烁
	if(SegMode == 3)
	{
		if(++Time_100ms == 100)
		{
			Time_100ms = 0;
			LedFlash = !LedFlash;
		}
	}
	else
	{
		Time_100ms = 0;
		LedFlash = 0;
	}
	//按键长按计时
	if(KeyPressFlag)
	{
		if(++Time_1000ms >= 1000)
			Time_1000ms = 1000;
	}
	else
		Time_1000ms = 0;
}

void Timer2_Isr(void) interrupt 12
{
	if(++pwm_count == 10)
		pwm_count = 0;
	Motor(pwm_count < pwm_level);
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
	{IICProc,200,0},
	{WaveProc,160,0}
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
	SchedulerInit();
	Timer0_Init();
	Timer1_Init();
	Timer2_Init();
	while(1)
	{
		SchedulerRun();
	}
}