#include <STC15F2K60S2.H>
#include "ds18b20.h"
#include "wave.h"
#include "init.h"
#include "led.h"
#include "key.h"
#include "seg.h"
#include "iic.h"

typedef unsigned char u8;
typedef unsigned int u16;

/*按键*/
idata u8 keyVal,keyDown,keyUp,keyOld;
idata bit keyPressFlag;//双按键同时按下标志位
idata u16 time2000ms;  //双按键长按2s计时
/*数码管*/
idata u8 segPos;
pdata u8 segBuf[8] = {10,10,10,10,10,10,10,10};
idata u8 segMode;   //0-环境 1-运动 2-参数 3-统计
/*指示灯*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata u8 time100ms;  		//计时100msL8闪烁
idata bit ledFlash;  		//L8闪烁标志位
idata u16 relayCount; 	//继电器吸合次数
idata bit relayHasCount;//继电器吸合一次只计数一次
/*温度*/
idata u8 tem;  				//环境温度
idata bit temHigFlag; //高温标志位
/*AD*/
idata u8 light; 	  //光敏电阻放大10倍
idata u8 lightLevel;//光照强度等级
/*超声波*/
idata u16 time1s;						//1s采集1次数据
idata u16 time2s; 					//计时2s
idata u16 time3s; 					//锁定状态时计时3s

idata u8 distance; 					//距离数据（1s采集1次）
idata u8 distanceLast;			//上一次的距离数据
idata u8 diff;							//两次距离的差值
idata u8 distanceMode = 1;	//运动状态（根据diff来判断）
idata u8 distanceLastMode;	//上一次的运动状态

idata bit distanceLock;			//刚上电2s时不采集距离数据
idata bit distanceFlag;			//距离数据1s采集1次标志位
idata bit distanceModeLock;	//运动状态锁定
idata bit close;            //接近判定标志位
/*参数*/
idata u8 keySlow;
idata u8 segSlow;
idata u8 iicSlow;
idata u8 temSlow;
idata u8 waveSlow;
idata u8 setDis[2] = {30,30}; //温度参数/距离参数控制值
idata u8 setCtr[2] = {30,30}; //温度参数/距离参数改变值
idata bit setCtrMode;         //温度参数/距离参数改变值索引

void keyProc()
{
	if(keySlow) return;
	keySlow = 1;
	
	keyVal = keyDisp();
	keyDown = keyVal & ~keyOld;
	keyUp = ~keyVal & keyOld;
	keyOld = keyVal;
	//双按键处理
	if(keyDown == 89 && segMode == 3)
		keyPressFlag = 1;
	if(time2000ms == 2001) //长按超过2s
	{
		relayCount = 0;
		keyPressFlag = 0;
		time2000ms = 0;
	}
	if(keyUp == 89)
	{
		keyPressFlag = 0;
		time2000ms = 0;
	}
	
	switch(keyDown)
	{
		case 4:
			if(++segMode == 4)
				segMode = 0;
			if(segMode == 3)
			{
				setCtrMode = 0;
				setDis[0] = setCtr[0];
				setDis[1] = setCtr[1];
			}
		break;
			
		case 5:
			if(segMode == 2)
				setCtrMode = !setCtrMode;
		break;
			
		case 8://参数加 30~80 
			if(segMode == 2)
			{
				if(!setCtrMode)//温度+1
				{
					if(++setCtr[0] == 81)
						setCtr[0] = 80;
				}
				else//距离+5
				{
					setCtr[1] += 5;
					if(setCtr[1] == 85)
						setCtr[1] = 80;
				}
			} 
		break;
			
		case 9://参数加 30~80 
			if(segMode == 2)
			{
				if(!setCtrMode)//温度-1
				{
					if(--setCtr[0] == 19)
						setCtr[0] = 20;
				}
				else//距离-5
				{
					setCtr[1] -= 5;
					if(setCtr[1] == 15)
						setCtr[1] = 20;
				}
			} 
		break;
	}
}

void segProc()
{
	if(segSlow) return;
	segSlow = 1;
	
	switch(segMode)
	{
		case 0:
			segBuf[0] = 11;  //C
			segBuf[1] = tem / 10;  
			segBuf[2] = tem % 10; 
			segBuf[4] = 10; 
			segBuf[5] = 10; 
			segBuf[6] = 12;  //n
			segBuf[7] = lightLevel;  
		break;
		
		case 1:
			segBuf[0] = 13;  //L
			segBuf[1] = distanceMode;  
			segBuf[2] = 10; 
		segBuf[5] = distance / 100 ? distance / 100 : 0;
			segBuf[6] = distance / 10 % 10;
			segBuf[7] = distance % 10;  
		break;
		
		case 2:
			segBuf[0] = 14;  //P
		segBuf[1] = !setCtrMode ? 11 : 13;  //C/L 
			segBuf[2] = 10; 
			segBuf[5] = 10;
			segBuf[6] = setCtr[setCtrMode] / 10;
			segBuf[7] = setCtr[setCtrMode] % 10; 
		break;
		
		case 3:
			segBuf[0] = 12;  //n
			segBuf[1] = 11;  //C
		
		segBuf[4] = relayCount / 1000 ? relayCount / 1000 : 10; 
		segBuf[5] = (segBuf[4]==10&&relayCount/100%10==0) ? 10 : relayCount/100%10;
		segBuf[6] = (segBuf[5]==10&&relayCount/10%10==0) ? 10 : relayCount/10%10;
			segBuf[7] = relayCount % 10; 
		break;
	}
}

void ledProc()
{
	//上电未满2s直接退出led处理函数
	if(!distanceLock)
		return;
	
	//继电器
	temHigFlag = (tem > setDis[0]);
	if(temHigFlag && close && !relayHasCount)
	{
		relayDisp(1);
		relayCount++;
		relayHasCount = 1;//继电器吸合已经计数了
	}
	if(!temHigFlag || !close)
	{
		relayDisp(0);
		relayHasCount = 0;//重置计数标志位
	}
	
	//L1~L4
	if(close)
	{
		switch(lightLevel)
		{
			case 1:
				ucLed[0] = 1;
				ucLed[1] = ucLed[2] = ucLed[3] = 0;
			break;
			
			case 2:
				ucLed[0] = ucLed[1] = 1;
				ucLed[2] = ucLed[3] = 0;
			break;
			
			case 3:
				ucLed[0] = ucLed[1] = ucLed[2] = 1;
				ucLed[3] = 0;
			break;
			
			case 4:
				ucLed[0] = ucLed[1] = ucLed[2] = ucLed[3] = 1;
			break;
		}
	}
	else
		ucLed[0] = ucLed[1] = ucLed[2] = ucLed[3] = 0;
	//L8
	if(distanceMode == 1)
		ucLed[7] = 0;
	else if(distanceMode == 2)
		ucLed[7] = 1;
	else 
		ucLed[7] = ledFlash;
}

void iicProc()
{
	if(iicSlow) return;
	iicSlow = 1;
	
	light = lightRead() / 51.0 * 10;
	/*光照强度转化*/
	if(light < 5)				//光照等级4:V<0.5->v*10<5
		lightLevel = 4;
	else if(light < 20) //光照等级3:V<2->v*10<20
		lightLevel = 3;
	else if(light < 30) //光照等级4:V<3->v*10<30
		lightLevel = 2;
	else								//光照等级4:V>=3
		lightLevel = 1;
}


void waveProc()
{
	if(waveSlow) return;
	waveSlow = 1;
	
	//上电未满2s直接退出距离处理函数
	if(!distanceLock)
		return;
	
	//采集数据
	if(!distanceFlag)//保证1s采集1次距离
	{	
		distance = waveGet();
		close = distance < setDis[1]; //距离值低于距离参数触发接近判定
		distanceFlag = 1;
		
		//状态锁定时不判断运动状态变化
		if(!distanceModeLock)
		{
			diff = (distance>distanceLast) ? distance-distanceLast : distanceLast-distance;
			distanceLast = distance;
			//差值转换运动状态
			if(diff < 5)			//静止（L1）
				distanceMode = 1;
			else if(diff < 10)//徘徊（L2）
				distanceMode = 2;
			else							//运动（L3）
				distanceMode = 3;
			
			//两次运动状态不同时锁定运动状态
			if(distanceLastMode != distanceMode)
				distanceModeLock = 1;
			distanceLastMode = distanceMode;//更新运动状态
		}
	}
}

void temProc()
{
	if(temSlow) return;
	temSlow = 1;
	
	tem = temRead();
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
	if(++keySlow == 10) keySlow = 0;
	if(++segSlow == 90) segSlow = 0;
	if(++waveSlow == 160) waveSlow = 0;
	if(++iicSlow == 160) iicSlow = 0;
	if(++temSlow == 160) temSlow = 0;
	if(++segPos == 8) segPos = 0;
	segDisp(segPos, segBuf[segPos]);
	ledDisp(ucLed);
	//上电计时2s
	if(!distanceLock)
	{
		if(++time2s == 2000)
			distanceLock = 1;
	}
	//1s采集1次数据
	if(distanceFlag)
	{
		if(++time1s == 1000)
		{
			time1s = 0;
			distanceFlag = 0;
		}
	}
	//状态锁定计时3s
	if(distanceModeLock)
	{
		if(++time3s == 3000)
		{
			time3s = 0;
			distanceModeLock = 0;
		}
	}
	//L8闪烁
	if(distanceMode == 3)
	{
		if(++time100ms == 100)
		{
			time100ms = 0;
			ledFlash = !ledFlash;
		}
	}
	else
	{
		time100ms = 0;
		ledFlash = 0;
	}
	//双按键长按2s
	if(keyPressFlag)
	{
		if(++time2000ms >= 2000)
			time2000ms = 2001;
	}
}

void main()
{
	systemInit();
	waveInit();
	temRead();
	Timer0_Init();
	while(1)
	{
		ledProc();
		keyProc();
		segProc();
		iicProc();
		temProc();
		waveProc();
	}
}