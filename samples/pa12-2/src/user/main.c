#include <STC15F2K60S2.H>
#include<init.h>
#include <key.h>
#include <seg.h>
#include <led.h>
#include <iic.h>

typedef unsigned char u8;
typedef unsigned int u16;

/*按键*/
idata u8 KeySlow;
idata u8 KeyVal, KeyDown, KeyUp, KeyOld;
idata bit KeyPress;   //长按标志位
idata u16 Time_1000ms;//长按1s
/*数码管*/
idata u8 SegSlow;
idata u8 SegMode;     //0-温度显示 1-参数设置 2-DA输出
idata u8 SegPos;
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
/*LED*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata bit led_off;    //led关闭标志位 1-有效
/*AD*/
idata u8 AdSlow;
idata bit mode;       //通道选择 0-通道1 1-通道2
idata u16 RD1_100x;   //光敏电阻电压放大100倍
idata u16 RB2_100x;   //变压器电压放大100倍
idata u16 ad_old = 500;//电压缓存数据，以防刚上电就点亮Led，赋最大值
/*频率*/
idata u16 freq, Time_1s;
idata bit lock;       //频率锁
idata u16 T;          //周期
idata u16 freq_old = 36000;//频率缓存数据，以防刚上电就点亮Led，赋最大值

/*
*关于为什么频率缓存数据和电压缓存数据要赋最大值的问题：
*题目要求大于缓存电压的时候L1，L2点亮
*刚上电时，缓存数据还没存，默认是0，所以L1和L2都会亮
*/

void KeyProc()
{
	if(KeySlow) return;
	KeySlow = 1;
	
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	//按键S7
	if(KeyDown == 7)
	{
		KeyPress = 1;//按下，标识位有效，定时器开始计时
	}
	if(KeyUp == 7)//松手，标志位无效，定时器停止计时
	{
		KeyPress = 0;
		//长按关闭Led
		if(Time_1000ms == 1000)
			led_off = !led_off;
		//短按保存频率
		else
			freq_old = freq;
		
		Time_1000ms = 0;//清空定时变量
	}
	
	
	switch(KeyDown)
	{
		case 4://页面切换
			if(++SegMode == 3)
				SegMode = 0;
			//每次进入电压页面默认通道1
			if(SegMode == 2)
				mode = 0;
		break;
			
		case 5://切换通道，电压页面有效
				if(SegMode == 2)
					mode = !mode;
		break;
				
		case 6://保存通道3的电压
			ad_old = RB2_100x;
		break;
		
	}
}

void SegProc()
{
	if(SegSlow) return;
	SegSlow = 1;
	
	switch(SegMode)
	{
		case 0://频率显示
			SegBuf[0] = 12;									//F
			SegBuf[1] = 10;
			SegBuf[2] = 10;
		//使用三目运算符判断高位熄灭
		if(lock)	
		{
			SegBuf[3] = (freq/10000%10) ? freq/10000%10 : 10;		
			SegBuf[4] = (freq/1000%10==0 && SegBuf[3]==10) ? 10 : freq/1000%10;		
			SegBuf[5] = (freq/100%10==0 && SegBuf[4]==10) ? 10 : freq/100%10;	
			SegPoint[5] = 0;
			SegBuf[6] = (freq/10%10==0 && SegBuf[5]==10) ? 10 : freq/10%10;		
			SegBuf[7] = freq % 10;
		}
		break;
		
		case 1://周期显示
			//T=1/f(T:s,f:hz)->us: T = 10^6/f
			T= 1000000 / freq;
			SegBuf[0] = 13;									//N
		SegBuf[1] = (T/1000000%10) ? T/1000000%10 : 10;		
		SegBuf[2] = (T/100000%10==0 && SegBuf[1]==10) ? 10 : T/100000%10;		
		SegBuf[3] = (T/10000%10==0 && SegBuf[2]==10) ? 10 : T/10000%10;		
		SegBuf[4] = (T/1000%10==0 && SegBuf[3]==10) ? 10 : T/1000%10;			
		SegBuf[5] = (T/100%10==0 && SegBuf[4]==10) ? 10 : T/100%10;		 
		SegBuf[6] = (T/10%10==0 && SegBuf[5]==10) ? 10 : T/10%10;		
		SegBuf[7] = T % 10;			
		break;
		
		case 2:
			SegBuf[0] = 14;									//U
			SegBuf[1] = 11;                 //-
			SegBuf[2] = !mode ? 1 : 3; 
			SegBuf[3] = 10; 
			SegBuf[4] = 10; 
			SegBuf[5] = !mode ? RD1_100x / 100 : RB2_100x / 100; 
			SegPoint[5] = 1;
			SegBuf[6] = !mode ? RD1_100x / 10 % 10 : RB2_100x / 10 % 10; 
			SegBuf[7] = !mode ? RD1_100x % 10 : RB2_100x % 10;        
		break;
	}
}

void AdProc()
{
	if(AdSlow) return;
	AdSlow = 1;
	
	//底层已经做了“读取两次丢弃第一次”的处理，这边直接读就行
	RD1_100x = AdRead(0x01) * 100 / 51;
	RB2_100x = AdRead(0x03) * 100 / 51;
}

void LedProc()
{
	u8 i;
	/*LED*/
	if(!led_off)
	{
		ucLed[0] = (RB2_100x > ad_old);
		ucLed[1] = (freq > freq_old);
		for(i = 0; i < 3; i++)
			ucLed[2+i] = (i == SegMode);
	}
	else
	{
		ucLed[0] = 0;
		ucLed[1] = 0;
		ucLed[2] = 0;
		ucLed[3] = 0;
		ucLed[4] = 0;
	}
}

void Timer0_Init(void)		//1毫秒@12.000MHz
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
	ET1 = 1;
	EA = 1;
}


void Timer1_Isr(void) interrupt 3
{
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 10) SegSlow = 0;
	if(++AdSlow == 10) AdSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	if(++Time_1s == 1000)
	{
		lock = 1;//解锁
		Time_1s = 0;
		freq = (TH0 << 8) | TL0;
		TH0 = TL0 = 0;
	}
	//长按
	if(KeyPress)
	{
		if(++Time_1000ms >= 1000)
			Time_1000ms = 1000;
	}
}

void main()
{
	SystemInit();
	Timer0_Init();
	Timer1_Init();
	while(1)
	{
		LedProc();
	  KeyProc();
		SegProc();
		AdProc();
	}
}