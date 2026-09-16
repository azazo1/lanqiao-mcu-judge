#include <STC15F2K60S2.H>
#include <Init.h>
#include <led.h>
#include <key.h>
#include <iic.h>
#include <seg.h>
#include <wave.h>

typedef unsigned char u8;
typedef unsigned int u16;
typedef unsigned long int u32;
/*按键*/
idata u8 KeySlow;
idata u8 KeyVal,KeyDown,KeyOld;
/*数码管*/
idata u8 SegMode;
idata u8 SegPos;
idata u8 SegSlow;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
/*RB2*/ 
idata u8 IICSlow;
idata u16 RB2_100x;              //变压器电压放大100倍（原因：不想用float型变量）
idata u8 set_10x[2] = {45,5};    //参数放大10倍，初始值：45 和 5，取值范围：5~50
idata u8 set_ctl_10x[2] = {45,5};//参数在参数调整中的值
idata bit set_mode;              //0-选择参数上限 1-选择参数下限
/*超声波*/
idata u8 WaveSlow;
idata u8 distance;               //超声波测量值
idata bit distance_flag;         //超声波连续测距启动标志位，默认不启动
/*Led*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata u8 Time_100ms;             //定时100msLed闪烁
idata u8 LedFlash;               //指示灯闪烁标志位

void KeyProc()
{
	if(KeySlow)	return;
	KeySlow = 1;
	
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyOld = KeyVal;
	switch(KeyDown)
	{
		case 4:
			if(++SegMode == 3)
				SegMode = 0;
			if(SegMode == 2)
			{
				set_mode = 0; 						 	//默认电压上限参数
				set_ctl_10x[0] = set_10x[0];//保存数据，不写也没什么影响
				set_ctl_10x[1] = set_10x[1];//保存数据，不写也没什么影响
			}
			if(SegMode == 0)
			{
				set_10x[0] = set_ctl_10x[0];//参数生效
				set_10x[1] = set_ctl_10x[1];//参数生效
			}
		break;
			
		case 5://选择参数
			if(SegMode == 2)
				set_mode = !set_mode;
		break;
		
		case 6://加
			if(SegMode == 2)
			{
				set_ctl_10x[set_mode] += 5;
				if(set_ctl_10x[set_mode] == 55)
					set_ctl_10x[set_mode] = 5;
			}
		break;
			
		case 7://减
			if(SegMode == 2)
			{
				set_ctl_10x[set_mode] -= 5;
				if(set_ctl_10x[set_mode] == 0)
					set_ctl_10x[set_mode] = 50;
			}
		break;
	}
}

void SegProc()
{
	if(SegSlow)	return;
	SegSlow = 1;
	
	//数码管
	switch(SegMode)
	{
		case 0://电压页面
			SegBuf[0] = 12;									//U
			SegBuf[3] = 10; 
			SegBuf[4] = 10;		
			SegBuf[5] = RB2_100x / 100;			//百位
			SegPoint[5] = 1;								//小数点使能
			SegBuf[6] = RB2_100x / 10 % 10; //十位
			SegBuf[7] = RB2_100x % 10;			//个位
			SegPoint[3] = SegPoint[6] = 0;
		break;
		
		case 1://测距页面
			SegBuf[0] = 14;//L	
			SegBuf[5] = distance_flag ? (distance / 100 ? distance / 100 : 10) : 15;
			SegBuf[6] = distance_flag ? distance / 10 % 10 : 15; 
			if(SegBuf[5] == 10)
				if(!SegBuf[6])
					SegBuf[6] = 10;
			SegBuf[7] = distance_flag ? distance % 10 : 15;	
			SegPoint[5] = 0;
		break;
		
		case 2://参数界面
			SegBuf[0] = 13;											   //P
			SegBuf[3] = set_ctl_10x[0] / 10 % 10;  //上限参数
			SegBuf[4] = set_ctl_10x[0] % 10;			 //上限参数
			SegBuf[5] = 10;
			SegBuf[6] = set_ctl_10x[1] / 10 % 10;  //下限参数
			SegBuf[7] = set_ctl_10x[1] % 10;		   //下限参数
			SegPoint[3] = SegPoint[6] = 1;
		break;
	}
}

void WaveProc()
{
	if(WaveSlow) return;
	WaveSlow = 1;
	//超声波数据处理
	distance = WaveCm() + 2;
}

void LedProc()
{
	u8 i;
	/*Led*/
	for(i = 0; i < 3; i++)
		ucLed[i] = (i == SegMode);
	ucLed[7] = (distance_flag & LedFlash);
}

void IICProc()
{
	idata u8 dat;
	if(IICSlow) return;
	IICSlow = 1;
	//AD数据处理
	RB2_100x = AdRead() * 100 / 51;
	distance_flag = (RB2_100x < set_10x[0] * 10 && RB2_100x > set_10x[1] * 10);
	//DA数据处理
	if(distance_flag)
	{
		if(distance <= 20)
			dat = 1;
		else if(distance >= 80)
			dat = 5;
		else
			dat = 1.0/15*distance+1-1.0/15*20;
	}
	else
		dat = 0;
	DaWrite(dat*51);
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
	//减速函数
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 100) SegSlow = 0;
	if(++IICSlow == 160) IICSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	if(++WaveSlow == 160) WaveSlow = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	
	if(distance_flag)//测距启动
	{
		if(++Time_100ms == 100)
		{
			Time_100ms = 0;
			LedFlash = !LedFlash;
		}
	}
	else//测距未启动
	{
		LedFlash = 0;
		Time_100ms = 0;
	}
}

void main()
{
	SystemInit();
	Timer0_Init();
	while(1)
	{
		KeyProc();
		SegProc();
		LedProc();
		IICProc();
		WaveProc();
	}
}
