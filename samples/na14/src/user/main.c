#include <STC15F2K60S2.H>
#include "Init.h"
#include "LED.h"
#include "Key.h"
#include "Seg.h"
#include "ds18b20.h"
#include "Wave.h"
#include "DAC.h"

/* 变量 */
idata unsigned long int systick;
pdata unsigned char ucLed[8] = {0, 0, 0, 0, 0, 0, 0, 0};
idata unsigned char SegPos;
pdata unsigned char SegBuf[8] ={10, 10, 10, 10, 10, 10, 10, 10};
pdata unsigned char SegPoint[8] = {0, 0, 0, 0, 0, 0, 0, 0};
idata unsigned char KeyVal, KeyDown, KeyUp, KeyOld;

idata float Tem;//温度

idata unsigned char seg_mode;										//0-测距 1-参数 2-工厂
idata unsigned char wave;												//超声波
idata unsigned char wave_set = 40, tem_set = 30;//距离(10~90)、温度参数(0~80)
idata unsigned char factory_mode;								//工厂模式
idata unsigned char dac_limited = 10;						//DAC下限(1~20)，使用时范围为0.1~2
idata unsigned char Time_100ms;									//定时100ms用于Led闪烁
pdata unsigned char distance_arr[12];           //记录数据存放数组
idata unsigned char distance_arr_index;         //数组指针

idata signed char calibration;//校准值 范围：-90~90

idata unsigned int v = 340;		 //超声波传播速度 10~9990
idata unsigned int Time_6000ms;//定时6s
idata unsigned int Time_500ms; //定时0.5s
idata unsigned int Time_2000ms;//定时2s

idata bit wave_mode;	//测距模式 0-cm 1-m
idata bit set_mode;		//参数模式 0-距离 1-温度
idata bit led_flash;	//LED闪烁
idata bit record_flag;//数据开始记录
idata bit rocord_over;//数据记录完毕
idata bit da_output;  //数据开始输出
idata bit press_long; //长按

void Key_Proc()
{
	unsigned char i;
	KeyVal = Key_Disp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	if(record_flag)
		return;
	
	if(KeyOld == 89)
		press_long = 1;
	if(Time_2000ms >= 2000)//恢复出厂设置
	{
		v = 340;
		seg_mode = 0;
		wave_mode = 0;
		set_mode = 0;
		factory_mode = 0;
		wave_set = 40;
		tem_set = 30;
		dac_limited = 10;
		calibration = 0;
		press_long = 0;
		for(i = 0; i < 8; i++)
			ucLed[i] = 0;
	}
	if(KeyUp == 89)
		press_long = 0;
	
	switch(KeyDown)
	{
		case 4:
			seg_mode++;
			if(seg_mode == 3)
			{
				seg_mode = 0;
				wave_mode = 0;
				set_mode = 0;
				factory_mode = 0;
			}
		break;
			
			
		case 5:
			if(!seg_mode)
				wave_mode ^= 1;
			else if(seg_mode == 1)
				set_mode ^= 1;
			else
			{
				factory_mode++;
				if(factory_mode == 3)
					factory_mode = 0;
			}
		break;
		
			
		case 8:
			switch(seg_mode)
			{
				case 0:
					if(!record_flag)
						record_flag = 1;//数据开始记录
				break;
				
				case 1:
					if(!set_mode)
					{
						wave_set += 10;
						if(wave_set >= 100)
							wave_set = 90;
					}
					else
					{
						tem_set += 1;
						if(tem_set >= 81)
							tem_set = 80;
					}
				break;
				
				case 2:
					switch(factory_mode)
					{
						case 0:
							calibration += 5;
							if(calibration == 95)
								calibration = 90;
						break;
								
						case 1:
							v += 10;
							if(v > 9990)
								v = 9990;
						break;
								
						case 2:
							dac_limited += 1;
							if(dac_limited == 21)
								dac_limited = 20;
						break;
					}						
				}
		break;
			
		case 9:
			switch(seg_mode)
			{
				case 0:
					if(rocord_over)
						da_output = 1;
				break;
				
				case 1:
					if(!set_mode)
					{
						wave_set -= 10;
						if(wave_set == 0)
							wave_set = 10;
					}
					else
					{
						tem_set -= 1;
						if(tem_set > 100)
							tem_set = 0;
					}
				break;
					
				case 2:
					switch(factory_mode)
					{
						case 0:
							calibration -= 5;
							if(calibration == -95)
								calibration = -90;
						break;
							
						case 1:
							v -= 10;
							if(v == 0)
								v = 10;
						break;
							
						case 2:
							dac_limited -= 1;
							if(dac_limited == 0)
								dac_limited = 1;
						break;
					}
				break;
			}
		break;
	}
}

void Seg_Proc()
{
	unsigned char i;
	
	switch(seg_mode)
	{
		case 0:
			SegBuf[0] = (unsigned char)Tem / 10;
			SegBuf[1] = (unsigned char)Tem % 10;
			SegBuf[2] = (unsigned int)(Tem * 10) % 10;
			SegBuf[3] = 11;
			SegBuf[4] = 10;
			SegBuf[5] = wave_mode ? 0 : 10;
			SegBuf[6] = wave / 10;
			SegBuf[7] = wave % 10;
			SegPoint[1] = 1;
			SegPoint[5] = wave_mode ? 1 : 0;
			SegPoint[6] = 0;
		break;
		
		case 1:
			SegPoint[1] = SegPoint[5] = 0;
			SegBuf[0] = 12;
			SegBuf[1] = set_mode ? 2 : 1;
			SegBuf[2] = 10;
			SegBuf[3] = 10;
			SegBuf[4] = 10;
			SegBuf[5] = 10;
			SegBuf[6] = set_mode ? tem_set / 10 : wave_set / 10;
			SegBuf[7] = set_mode ? tem_set % 10 : wave_set % 10;
		break;
		
		case 2:
			SegBuf[0] = 13;
			SegBuf[1] = factory_mode + 1;
			SegBuf[2] = 10;
			SegBuf[3] = 10;
			switch(factory_mode)
			{
				case 0:
					SegPoint[6] = 0;
					SegBuf[4] = 10;
					if(calibration >= 0)//为正数时
					{
						SegBuf[5] = 10;
						SegBuf[6] = (calibration / 10) ? calibration / 10 : 10;//十位为0时高位熄灭
						SegBuf[7] = calibration % 10;
					}
					else//为负数时
					{
						unsigned char positive = -calibration;
						if(positive > 9)
						{
							SegBuf[5] = 11;
							SegBuf[6] = positive / 10;
							SegBuf[7] = positive % 10;
						}
						else
						{
							SegBuf[5] = 10;
							SegBuf[6] = 11;
							SegBuf[7] = positive;
						}
					}
				break;
					
				case 1:
					SegBuf[4] = v / 1000;
					SegBuf[5] = v / 100 % 10;
					SegBuf[6] = v / 10 % 10;
					SegBuf[7] = v % 10;
					i = 4;
					while(!SegBuf[i])
					{
						SegBuf[i] = 10;
						i ++;
						if(i == 7)
							break;
					}
				break;
					
				case 2:
					SegBuf[4] = 10;
					SegBuf[5] = 10;
					SegBuf[6] = dac_limited / 10;
					SegBuf[7] = dac_limited % 10;
					SegPoint[6] = 1;
				break;	
			}
		break;
	}
}

void Led_Proc()
{
	unsigned char i;
	bit temp;
	
	switch(seg_mode)
	{
		case 0:
			for(i = 0; i < 7; i++)
				ucLed[i] = (wave >> i) & 0x01;
		break;
		
		case 1:
			for(i = 0; i < 7; i++)
				ucLed[i] = 0;
			ucLed[7] = 1;
		break;
		
		case 2:
			ucLed[7] = 0;
			ucLed[0] = led_flash;
		break;
	}
	LED_Disp(ucLed);
	
	temp = ((wave >= wave_set - 5) && (wave <= wave_set + 5) && (Tem < tem_set));
	Relay(temp);
}

void DS18B20_Proc()
{
	Tem = Tem_Read();
}

void DAC_Proc()
{
	
	if(da_output)
	{
		unsigned char i;
		float dac = dac_limited * 0.1;
		unsigned int distance_ave = 0;
		for(i = 0; i < 12; i++)
			distance_ave += distance_arr[distance_arr_index];
		distance_ave /= distance_arr_index;
		
		if(distance_ave < 10)
			DAC(dac * 51);
		else if(distance_ave > 90)
			DAC(255);
		else
		{
			unsigned char temp = (5 - dac) * distance_ave +  (5 - (5 - dac) * 90);
			DAC(temp * 51);
		}
			
	}
}

void Wave_Proc()
{

	unsigned char positive = -calibration;
	if(calibration < 0)
		wave = Wave_Cm(v) - positive;
	else
		wave = Wave_Cm(v) + calibration;
	
	if(record_flag)
	{
		distance_arr[distance_arr_index] =  wave;
		if(distance_arr_index == 12)
		{
			distance_arr_index = 11;
			rocord_over = 1;
		}
	}
	else
	{
		distance_arr_index = 0;
	}
}


void Timer0_Init(void)		//1毫秒@12.000MHz
{
	AUXR &= 0x7F;			//定时器时钟12T模式
	TMOD &= 0xF0;			//设置定时器模式
	TL0 = 0x18;				//设置定时初始值
	TH0 = 0xFC;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
	ET0 = 1;
	EA = 1;
}

void Timer0_Isr(void) interrupt 1
{
	systick++;
	if(++SegPos == 8)	
		SegPos = 0;
	Seg_Disp(SegPos, SegBuf[SegPos], SegPoint[SegPos]);
	if(++Time_100ms == 100)
	{
		Time_100ms = 0;
		led_flash = !led_flash;
	}
	if(record_flag)
	{
		if(++Time_6000ms == 6000)
		{
			Time_6000ms = 0;
			record_flag = 0;
		}
		if(++Time_500ms == 500)
		{
			Time_500ms = 0;
			distance_arr_index++;
		}
	}
	if(press_long)
	{
		if(++Time_2000ms >= 2000)
			Time_2000ms = 2001;
	}
	else
		Time_2000ms = 0;
}

typedef struct 
{
	void (*p)(void);
	unsigned long int rate_ms;
	unsigned long int last_ms;
}Task;

idata Task Scheduler_Task[] = {
	{Key_Proc, 10, 0},
	{Led_Proc, 1, 0},
	{DS18B20_Proc, 300, 0},
	{DAC_Proc, 80, 0},
	{Wave_Proc, 500, 0},
	{Seg_Proc, 90, 0},
};

idata unsigned char task_num;

void Scheduler_Init(void)
{
	task_num = sizeof(Scheduler_Task) / sizeof(Task);
}

void Scheduler_Run(void)
{
	unsigned char i;
	for(i = 0; i < task_num; i++)
	{
		unsigned long int now_tick = systick;
		if(now_tick >= Scheduler_Task[i].rate_ms + Scheduler_Task[i].last_ms)
		{
			Scheduler_Task[i].last_ms = now_tick;
			Scheduler_Task[i].p();
		}
	}
}

void main()
{
	Init();
	Tem_Read();
	Timer0_Init();
	Scheduler_Init();
	
	while(1)
	{
		Scheduler_Run();
	}
}