#include <STC15F2K60S2.H>
#include <init.h>
#include <key.h>
#include <seg.h>
#include <led.h>
#include <ds1302.h>
#include <ds18b20.h>

typedef unsigned char u8;
typedef unsigned int u16;

/*按键*/
idata u8 KeySlow;
idata u8 KeyVal, KeyDown, KeyUp, KeyOld;
idata bit ctr;           //控制模式 0-温度控制 1-时间控制
/*时间*/
idata u8 RtcSlow;
pdata u8 Rtc[3] = {23,59,50};
idata bit rtc_mode;      //0-显示时-分 1-显示分-秒
/*温度*/
idata u8 TemSlow;
idata u16 tem_10x;       //温度放大10倍
idata u8 tem_set = 23;   //温度参数
/*数码管*/
idata u8 SegSlow;
idata u8 SegPos;
idata u8 SegMode;  			 //0-温度显示 1-时间显示 2-参数设置
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
/*LED*/
idata bit RelayFlag;     //继电器使能标志位
idata u16 Time_5s;       //时间控制模式下计时5s
idata bit HourTime;      //整时标志位
idata u8 Time_100ms;     //继电器吸合L3以100ms为间隔闪烁
idata bit L3Flash;       //指示灯L3点亮标志位
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};

void KeyProc()
{
	if(KeySlow) return;
	KeySlow = 1;
	
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	//长按单独判断
	if(SegMode == 1 && KeyOld == 17)//如果在时间页面并且S17长按，切换到分-秒显示
		rtc_mode = 1;
	if(KeyUp == 17 && rtc_mode)//分-秒显示下松开S17，回到时-分显示
		rtc_mode = 0;
	
	switch(KeyDown)
	{
		case 12://切换页面
			if(++SegMode == 3)
				SegMode = 0;
		break;
			
		case 13://切换工作模式，任意页面有效
			ctr = !ctr;
		break;
		
		case 16://加 参数设置页面有效
			if(SegMode == 2)
			{
				if(++tem_set == 100)
					tem_set = 99;
			}
		break;
			
		case 17://减 参数设置页面有效
			if(SegMode == 2)
			{
				if(--tem_set == 9)
					tem_set = 10;
			}
		break;
	}
}

void SegProc()
{
	if(SegSlow) return;
	SegSlow = 1;
	
	SegBuf[0] = 12;
	SegBuf[1] = SegMode + 1;
	switch(SegMode)
	{
		case 0://温度页面
			SegBuf[5] = tem_10x / 100;
			SegBuf[6] = tem_10x / 10 % 10;
			SegPoint[6] = 1;
			SegBuf[7] = tem_10x % 10;
		break;
		
		case 1://时间页面
			SegBuf[3] = !rtc_mode ? Rtc[0] / 10 : Rtc[1] / 10;
			SegBuf[4] = !rtc_mode ? Rtc[0] % 10 : Rtc[1] % 10;
			SegBuf[5] = 11;
			SegBuf[6] = !rtc_mode ? Rtc[1] / 10 : Rtc[2] / 10;
			SegPoint[6] = 0;
			SegBuf[7] = !rtc_mode ? Rtc[1] % 10 : Rtc[2] % 10;
		break;
			
		case 2://参数页面
			SegBuf[3] = 10;
			SegBuf[4] = 10;
			SegBuf[5] = 10;
			SegBuf[6] = tem_set / 10;
			SegBuf[7] = tem_set % 10;
		break;
	}
}

void LedProc()
{
	//整时判断（分和秒为0即整时）
	if(Rtc[1] == 0 && Rtc[2] == 0)
		HourTime = 1;
	
	/*继电器*/
	if(!ctr)//温度控制
		RelayFlag = (tem_10x > tem_set * 10);//温度大于温度参数，由于温度放大10倍
	else
	{
			RelayFlag = HourTime;
	}
	
	/*LED*/
	ucLed[0] = HourTime;
	ucLed[1] = !ctr;
	ucLed[2] = L3Flash;
}

void RtcProc()
{
	if(RtcSlow) return;
	RtcSlow = 1;
	GetRtc(Rtc);
}

void TemProc()
{
	if(TemSlow) return;
	TemSlow = 1;
	tem_10x = TemRead() * 10;
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
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 100) KeySlow = 0;
	if(++RtcSlow == 200) RtcSlow = 0;
	if(++TemSlow == 160) TemSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	Relay(RelayFlag);
	//整时：1.时间模式 继电器吸合5s后闭合 2.L1亮5s
	if(HourTime)
	{
		if(++Time_5s == 5000)
		{
			Time_5s = 0;
			HourTime = 0;
		}
	}
	//L3闪烁：继电器吸合
	if(RelayFlag)
	{
		if(++Time_100ms == 100)
		{
			Time_100ms = 0;
			L3Flash = !L3Flash;
		}
	}
	else//继电器闭合时熄灭L3并清空计时器
	{
		Time_100ms = 0;
		L3Flash = 0;
	}
}

void main()
{
	SystemInit();
	TemRead();
	SetRtc(Rtc);
	Timer0_Init();
	while(1)
	{
		LedProc();
		KeyProc();
		SegProc();
		RtcProc();
		TemProc();
	}
}