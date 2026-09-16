#include <STC15F2K60S2.H>
#include<init.h>
#include <key.h>
#include <seg.h>
#include <led.h>
#include <iic.h>
#include <ds18b20.h>

typedef unsigned char u8;
typedef unsigned int u16;

/*按键*/
idata u8 KeySlow;
idata u8 KeyVal, KeyDown, KeyUp, KeyOld;
/*数码管*/
idata u8 SegSlow;
idata u8 SegMode;     //0-温度显示 1-参数设置 2-DA输出
idata u8 SegPos;
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
/*LED&DA*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata float da_output;//DA输出值
idata bit mode;       //模式选择 0-模式1 1-模式2
/*温度*/
idata u8 TemSlow;
idata u16 tem_100x;   //温度放大100倍（体现两位小数）
idata u8 tem_set = 25;//温度参数，初值为25，题目没给出调整范围
idata u8 tem_set_ctl = 25;//由于温度调整在退出时生效，因此需要定义一个变化值

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
			if(++SegMode == 3)
				SegMode = 0;
			//退出参数设置页面时，温度数据生效
			if(SegMode == 2)
				tem_set = tem_set_ctl;
		break;
			
		case 5://切换模式，任意页面有效
			mode = !mode;
		break;
			
		case 8://减，参数设置页面有效
			if(SegMode == 1)
			{
				tem_set_ctl--;
			}
		break;
			
		case 9://加，参数设置页面有效
			if(SegMode == 1)
			{
				tem_set_ctl++;
			}
		break;
	}
}

void SegProc()
{
	if(SegSlow) return;
	SegSlow = 1;
	
	switch(SegMode)
	{
		case 0://温度显示
			SegBuf[0] = 12;									//C
		
			//例如25.16°，tem_100x为2516
		
			SegBuf[4] = tem_100x / 1000;		//2
			SegBuf[5] = tem_100x / 100 % 10;//5
			SegPoint[5] = 1;                //小数点
			SegBuf[6] = tem_100x / 10 % 10; //1
			SegBuf[7] = tem_100x % 10;			//6
		break;
		
		case 1://参数调整
			SegBuf[0] = 13;									//P
			SegBuf[4] = 10;		
			SegBuf[5] = 10;
			SegPoint[5] = 0;                
			SegBuf[6] = tem_set_ctl / 10 % 10; 
			SegBuf[7] = tem_set_ctl % 10;			
		break;
		
		case 2:
			SegBuf[0] = 14;									//R
			SegBuf[5] = (u8)da_output % 10;
			SegPoint[5] = 1;       
			SegBuf[6] = (u8)(da_output * 10) % 10; 
			SegBuf[7] = (u16)(da_output * 100) % 10;			
		break;
	}
}

void LedProc()
{
		u8 i;
	/*DAC*/
	if(!mode)
		da_output = (tem_100x < tem_set_ctl * 100) ? 0 : 5;
	else
	{
		if(tem_100x <= 2000)
			da_output = 1;
		else if(tem_100x >= 4000)
			da_output = 4;
		else 
			da_output = 3.0*tem_100x/2000-2; 
	}
	DaWrite(da_output*51);
	/*LED*/
	ucLed[0] = !mode;
	for(i = 0; i < 3; i++)
		ucLed[1+i] = (i == SegMode);
}

void TemProc()
{
	if(TemSlow) return;
	TemSlow = 1;
	tem_100x = TemRead() * 100;
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
	if(++SegSlow == 100) SegSlow = 0;
	if(++TemSlow == 160) TemSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
}

void main()
{
	SystemInit();
	Timer0_Init();
	while(1)
	{
		LedProc();
		KeyProc();
		SegProc();
		TemProc();
	}
}