#include <STC15F2K60S2.H>
#include <led.h>
#include <seg.h>
#include <init.h>
#include <iic.h>
#include <ds1302.h>
#include <key.h>

typedef unsigned char u8;
typedef unsigned int u16;
typedef signed char s8;
typedef signed int s16;

/*按键*/
idata u8 KeySlow;//按键延迟
idata u8 KeyVal,KeyUp,KeyDown,KeyOld;
/*数码管*/
idata u8 SegSlow;//数码管延迟
idata u8 SegPos;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
idata u8 SegMode;        //0-频率页面 1-参数页面 2-时间页面 3-回显页面
/*频率*/
idata u16 Time_1s;
idata u16 Freq;          //最终频率
idata u16 freq_max;      //最大频率
pdata u8 freq_max_rtc[3];//最大频率发生时间
idata bit echo_mode;     //回显模式
idata u16 PF = 2000;     //超限参数，初值为2000，范围1000~9000
idata s16 calibration;    //校准值，范围-900~900
idata bit SetMode;       //0-超限参数 1-校准值
idata bit freq_useful;   //校准后的频率正负标志位
/*时间*/
pdata u8 Rtc[3] = {0x23,0x59,0x50};//随便设
/*指示灯*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata u8 Time_200ms;  //定时200ms
idata u8 Time200ms;   //定时200ms
idata bit LedFlash;		//指示灯L1闪烁
idata bit L2Flash;    //指示灯L2闪烁

void KeyProc()
{
	if(KeySlow) return;
	KeySlow = 1;
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	switch(KeyDown)
	{
		case 4:
			if(++SegMode == 4)
				SegMode = 0;
			if(SegMode == 1)//切换到参数页面，处于超限参数
				SetMode = 0;
			else if(SegMode == 3)//切换到回显页面，处于最大频率
				echo_mode = 0;
		break;
			
		case 5:
			if(SegMode == 1)
				SetMode = !SetMode;
			if(SegMode == 3)
				echo_mode = !echo_mode;
		break;
			
		case 8://加
			if(SegMode == 1)//参数界面
			{
				if(!SetMode)
				{
					PF += 1000;
					if(PF == 10000)
						PF = 9000;
				}
				else
				{
					calibration += 100;
					if(calibration == 1000)
						calibration = 900;
				}
			}
		break;
			
		case 9://减
			if(SegMode == 1)//参数界面
			{
				if(!SetMode)
				{
					PF -= 1000;
					if(PF == 0)
						PF = 1000;
				}
				else
				{
					calibration -= 100;
					if(calibration == -1000)
						calibration = -900;
				}
			}
		break;
	}
}

void SegProc()
{
	u8 i;
	if(SegSlow) return;
	SegSlow = 1;
	
	GetRtc(Rtc);
	
	if(Freq > freq_max)
	{
		freq_max = Freq;
		GetRtc(freq_max_rtc);
	}
	
	switch(SegMode)
	{
		case 0:
			SegBuf[0] = 12;//F
			SegBuf[1] = 10;
			SegBuf[2] = 10;
			if(freq_useful)//频率无效
			{
				SegBuf[3] = 10;
				SegBuf[4] = 10;
				SegBuf[5] = 10;
				SegBuf[6] = 16;//L
				SegBuf[7] = 16;//L
			}
			else
			{
				SegBuf[3] = Freq / 10000 % 10;
				SegBuf[4] = Freq / 1000 % 10;
				SegBuf[5] = Freq / 100 % 10;
				SegBuf[6] = Freq / 10 % 10;
				SegBuf[7] = Freq % 10;
				i = 3;
				while(!SegBuf[i])
				{
					SegBuf[i] = 10;
					if(++i == 7)
						break;
				}
			}
		break;
			
		case 1:
			SegBuf[0] = 13;//P
			SegBuf[1] = SetMode ? 2 : 1;
			SegBuf[2] = 10;
			SegBuf[3] = 10;
			if(!SetMode)
			{
				SegBuf[4] = PF / 1000 % 10;
				SegBuf[5] = PF / 100 % 10;
				SegBuf[6] = PF / 10 % 10;
				SegBuf[7] = PF % 10;	
			}
			else
			{
				if(calibration > 0)
				{
					SegBuf[4] = 10;
					SegBuf[5] = calibration / 100 % 10;
					SegBuf[6] = calibration / 10 % 10;
					SegBuf[7] = calibration % 10;	
				}
				else if(calibration == 0)
				{
					SegBuf[4] = 10;
					SegBuf[5] = 10;
					SegBuf[6] = 10;
					SegBuf[7] = 0;	
				}
				else
				{
					s16 positive = - calibration;
					SegBuf[4] = 11;
					SegBuf[5] = positive / 100 % 10;
					SegBuf[6] = positive / 10 % 10;
					SegBuf[7] = positive % 10;	
				}
			}
		break;
			
		case 2:
			SegBuf[2] = SegBuf[5] = 11;
			for(i = 0; i < 3; i++)
			{
				SegBuf[3*i] = Rtc[i] / 16;
				SegBuf[3*i+1] = Rtc[i] % 16;
			}
		break;
			
		case 3:
			SegBuf[0] = 14;//H
			SegBuf[1] = echo_mode ? 15 : 12;
			SegBuf[2] = 10;
			if(!echo_mode)
			{
				SegBuf[3] = freq_max / 10000 % 10;
				SegBuf[4] = freq_max / 1000 % 10;
				SegBuf[5] = freq_max / 100 % 10;
				SegBuf[6] = freq_max / 10 % 10;
				SegBuf[7] = freq_max % 10;
				i = 3;
				while(!SegBuf[i])
				{
					SegBuf[i] = 10;
					if(++i == 7)
						break;
				}
			}
			else
			{
				for(i = 0; i < 3; i++)
				{
					SegBuf[2+2*i] = freq_max_rtc[i] / 16;//2 4 6
					SegBuf[3+2*i] = freq_max_rtc[i] % 16;
				}
			}
		break;
	}
}

void LedProc()
{
	/*DAC*/
	float dat;
	if(freq_useful)//最终频率为负，输出0
		dat = 0;
	else
	{
		if(Freq <= 500)
			dat = 1;
		else if(Freq >= PF)
			dat = 5;
		else 
			dat = 4.0/(PF-500)*Freq+1-4.0/(PF-500)*500;
	}
	DaWrite(dat*51);
	/*Led*/
	ucLed[0] = LedFlash;
	if(freq_useful)
		ucLed[1] = 1;
	else
		ucLed[1] = L2Flash;
}

void Timer0_Init(void)		//100微秒@12.000MHz
{
	TMOD &= 0xF0;			//设置定时器模式
	TMOD |= 0x05;
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

void Timer1_Isr(void) interrupt 3
{
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 100) SegSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	/*NE555*/
	if(++Time_1s == 1000)
	{
		Time_1s = 0;
		Freq = (TH0 << 8) | TL0;
		TH0 = TL0 = 0;
		Freq += calibration; //最终频率
		freq_useful = (Freq > 64000);
	}
	//处于频率页面
	if(!SegMode)
	{
		if(++Time_200ms == 200)
		{
			Time_200ms = 0;
			LedFlash = !LedFlash;
		}
	}
	else
	{
		LedFlash = 0;
		Time_200ms = 0;
	}
	/*L2*/
	if(Freq > PF)
	{
		if(++Time200ms == 200)
		{
			Time200ms = 0;
			L2Flash = !L2Flash;
		}
	}
	else
	{
		L2Flash = 0;
		Time200ms = 0;
	}
}

void main()
{
	SystemInit();
	SetRtc(Rtc);
	Timer0_Init();
	Timer1_Init();
	while(1)
	{
		KeyProc();
		SegProc();
		LedProc();
	}
}