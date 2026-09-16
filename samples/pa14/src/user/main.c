#include <STC15F2K60S2.H>
#include <ds18b20.h>
#include <ds1302.h>
#include <init.h>
#include <iic.h>
#include <led.h>
#include <key.h>
#include <seg.h>

typedef unsigned char u8;
typedef unsigned int u16;

/*延迟函数*/
idata u8 KeySlow;//按键延迟
idata u8 SegSlow;//数码管延迟
idata u8 TemSlow;//时钟延迟
idata u8 AdSlow; //AD延迟
idata u8 RtcSlow;//时钟延迟
/*按键*/
idata u8 KeyVal, KeyDown, KeyUp, KeyOld;
idata bit KeyPress;       //按键按下标志位
idata u16 Time_2s;        //按键长按2s
/*数码管*/
idata u8 SegPos;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
idata bit SegMode;        //0-数据页面 1-温湿度页面
idata u8 SegShowMode;     //数据页面分页面 0-时间页面 1-回显页面 2-参数页面
idata u8 ShowSetMode;     //回显页面分页面 0-温度回显 1-湿度回显 2-时间回显
/*LED*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
idata bit L4Flag;         //L4点亮标志位
idata u8 Time_100ms;      //100ms为闪烁间隔
/*频率*/
idata u16 freq, Time_1s;
idata u8 humidity;        //湿度数据
idata bit humidity_flag;  //湿度数据有效标志位
/*温度*/
idata u8 tem;             //实时温度
idata u8 tem_set = 30;    //温度参数 初值为30
idata bit tem_flag;       //采集温度>温度参数
/*时间*/
pdata u8 Rtc[3] = {0x23,0x59,0x50};
/*触发采集专用变量*/
idata u16 AD_100x;        //光敏电阻实时采集的值
idata u16 AD_100x_old;    //上一次光敏电阻采集的值(160ms前)
idata bit record;         //触发采集标志位
idata bit has_record;     //采集重复触发标志位
idata u16 Time_3s;        //触发采集3s后回到原页面
/*采集数据*/
idata u16 freq_capture;   //采集频率
idata u8 tem_capture;     //采集温度
idata u8 count_capture;   //触发采集的次数
idata u8 tem_max;         //最大温度
idata u8 tem_sum;         //温度总和 tem_sum += tem_capture
idata float tem_ave;      //平均温度 tem_ave = tem_sum / count_capture
idata u8 humidity_max;    //最大湿度 
idata u8 humidity_sum;    //湿度总和 humidity_sum += humidity
idata float humidity_ave; //平均湿度 humidity_ave = humidity_sum / count_capture
pdata u8 rtc_capture[2];  //触发采集时间 只记录时和分
/*指示灯L6*/
idata u8 tem_capture_old = 99;//上一次的采集温度，防止第一次就生效，赋值为第一次不可能等于的值
idata u8 humidity_old = 91;   //上一次的采集湿度，防止第一次就生效，赋值为第一次不可能等于的值
idata bit L6Flag;             //指示灯L6点亮标志位
idata bit tem_hig_flag;       //本次温度升高标志位
idata bit hum_hig_flag;       //本次湿度升高标志位

void KeyProc()
{
	if(KeySlow) return;
	KeySlow = 1;
	
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	if(SegMode) //触发采集时退出按键
		return;
	
	if(ShowSetMode == 2 && KeyDown == 9)
		KeyPress = 1;
	if(KeyUp == 9)
	{
  	KeyPress = 0;
		if(Time_2s == 2001)//长按清空所有记录的数据，注意不是回到初始状态！
		{
			count_capture = humidity_flag = freq_capture = 0;
			tem_capture = tem_max = tem_sum = tem_ave = 0;
			humidity_max = humidity_sum = humidity_ave = 0;
			rtc_capture[0] = rtc_capture[1] = 0;
			L6Flag = L4Flag = 0;
			tem_capture_old = 99;
			humidity_old = 91;
		}
		Time_2s = 0;
	}
	
	switch(KeyDown)
	{
		case 4:
			if(++SegShowMode == 3)//切换数据子页面
				SegShowMode = 0;
			if(SegShowMode == 2)//每次进入回显页面默认温度页面，可以在参数页面重置（其他页面也行）
				ShowSetMode = 0;
		break;
			
		case 5:
			if(SegShowMode == 1)//切换回显子页面
				if(++ShowSetMode == 3)
					ShowSetMode = 0;
		break;
				
		case 8://加
			if(SegShowMode == 2)//参数页面有效 取值范围：0~99
				if(++tem_set == 100)
					tem_set = 99;
		break;
				
		case 9://减
			if(SegShowMode == 2)//参数页面有效 取值范围：0~99
				if(--tem_set == 255)
					tem_set = 0;
		break;
	}
	
}

void SegProc()
{
	u8 i;
	if(SegSlow) return;
	SegSlow = 1;
	
	if(!SegMode)
	{
		switch(SegShowMode)
		{
			case 0:
				SegBuf[2] = SegBuf[5] = 11; //-
				for(i = 0; i < 3; i++)
				{
					SegBuf[3*i] = Rtc[i] / 16;
					SegBuf[3*i+1] = Rtc[i] % 16;
				}
			break;
				
			case 1:
				switch(ShowSetMode)
				{
					case 0:
						SegBuf[0] = 12; //C
						SegBuf[1] = 10; 
						SegPoint[6] = count_capture;//采集次数>0才显示小数点
					SegBuf[2] = count_capture ? tem_max / 10 : 10; 
					SegBuf[3] = count_capture ? tem_max % 10 :10; 
					SegBuf[4] = count_capture ? 11 : 10; //-
					SegBuf[5] = count_capture ? (u8)tem_ave / 10 : 10; 
					SegBuf[6] = count_capture ? (u8)tem_ave % 10 : 10; 
					SegBuf[7] = count_capture ? (u16)(tem_ave * 10) % 10 : 10;  
					break;
					
					case 1:
						SegBuf[0] = 13; //H
						SegBuf[1] = 10; 
						SegPoint[6] = count_capture;//采集次数>0才显示小数点
					SegBuf[2] = count_capture ? humidity_max / 10 : 10; 
					SegBuf[3] = count_capture ? humidity_max % 10 :10; 
					SegBuf[4] = count_capture ? 11 : 10; //-
					SegBuf[5] = count_capture ? (u8)humidity_ave / 10 : 10; 
					SegBuf[6] = count_capture ? (u8)humidity_ave % 10 : 10; 
					SegBuf[7] = count_capture ? (u16)(humidity_ave * 10) % 10 : 10;  
					break;
					
					case 2:
						SegBuf[0] = 14; //F
						SegPoint[6] = 0;
					SegBuf[1] = count_capture / 10;
					SegBuf[2] = count_capture % 10;
					SegBuf[3] = count_capture ? rtc_capture[0] / 16 : 10;
					SegBuf[4] = count_capture ? rtc_capture[0] % 16 : 10;
					SegBuf[5] = count_capture ? 11 : 10;
					SegBuf[6] = count_capture ? rtc_capture[1] / 16 : 10;
					SegBuf[7] = count_capture ? rtc_capture[1] % 16 : 10;
					break;
				}
			break;
				
			case 2:
				SegBuf[0] = 15; //P
				SegPoint[6] = 0;
				SegBuf[1] = 10;
				SegBuf[2] = 10;
				SegBuf[3] = 10;
				SegBuf[4] = 10;
				SegBuf[5] = 10;
				SegBuf[6] = (tem_set / 10) ? tem_set / 10 : 10;
				SegBuf[7] = tem_set % 10;
			break;
		}
	}
	else
	{
		SegBuf[0] = 16; //E
		SegBuf[1] = 10;
		SegBuf[2] = 10;
		SegBuf[3] = tem_capture / 10;
		SegBuf[4] = tem_capture % 10;
		SegBuf[5] = 11; //-
		SegBuf[6] = !humidity_flag ? humidity / 10 : 17;//A
		SegBuf[7] = !humidity_flag ? humidity % 10 : 17;//A
		SegPoint[6] = 0;
	}
}

void LedProc()
{
	tem_flag = (tem_capture > tem_set);
	if(!SegMode)
	{
		ucLed[0] = !SegShowMode; 		 //处于时间页面点亮
		ucLed[1] = SegShowMode == 1; //处于回显页面点亮
	}
	ucLed[2] = SegMode;                //处于温湿度页面点亮
	ucLed[3] = L4Flag;                 //采集温度大于温度参数时闪烁
	ucLed[4] = humidity_flag;          //采集到无效数据点亮
	ucLed[5] = !humidity_flag & L6Flag;//本次采集的温度湿度均升高(考虑湿度数据无效时应该熄灭)
}

void TemProc()
{
	if(TemSlow) return;
	TemSlow = 1;
	tem = TemRead();
}

void RtcProc()
{
	if(RtcSlow) return;
	RtcSlow = 1;
	GetRtc(Rtc);
}

void AdProc()
{
	if(AdSlow) return;
	AdSlow = 1;
	AD_100x = AdRead() / 51.0 * 100;
	/**
	* 触发采集判断：环境由亮到暗触发，持续处于暗状态不能连续触发。
	* 即实时光敏电阻的值<100(本次为暗)并且上一次的光敏电阻>100(上次为亮)触发采集，
	* 如果只写了if(AD_100x < 100 && AD_100x_old >= 100 && !record) 
	*           {
	*              record = 1;
	*              SegMode = 1;
	*           }
	* 定时器:
	*						if(record)
	*           {
	*             if(++Time_3s == 3000)
	*             {
	*             	Time_3s = 0;
	*               record = 0;
	*               SegMode = 0;
	*             }
	*           }
	* 这样写在当环境由亮到暗，触发采集，3秒后清除采集标志位，
	* 并不会重复触发采集，看起来是正确的，但是如果这样写，
	* 它就会“疯狂”进入下面的采集数据一直采集，
	* 为了避免这种情况就要加上防止数据重复采集标志位has_record了。
	**/
	if(AD_100x < 100 && AD_100x_old >= 100 && !record)
	{
		record = 1;    //采集触发
		SegMode = 1;   //切换到温湿度页面
	}
	AD_100x_old = AD_100x;
	
	//防止数据重复采集
	if(!has_record && record)//采集数据
	{
		has_record = 1;  //重复采集标志位生效，触发后将不会重复采集数据 
		/**
		* 采集数据时，如果湿度无效则其他数据均不采集，得先判断湿度是否有效。
		* 无论湿度是不是有效，温湿度页面都是要显示温度的，所以第一次采集，
		* 需要采集湿度和温度，等到湿度数据有效再采集其他数据。
		**/
		freq_capture = freq;
		tem_capture = tem;
		//测量的频率数据不在200~2000范围内认为频率数据无效，不进行采集数据
		humidity_flag = (freq_capture < 200 || freq_capture > 2000);
		
		if(!humidity_flag)//频率数据有效，开始转换湿度并采集其他数据
		{
			humidity = 2.0*freq_capture/45  + 10 - 400.0/45;//转换湿度
			
			//指示灯L6使能判断
			tem_hig_flag = tem_capture > tem_capture_old; //本次采集温度升高（触发次数>2生效）
			hum_hig_flag = humidity > humidity_old; //本次采集温度升高（触发次数>2生效）
			L6Flag = tem_hig_flag & hum_hig_flag;
			
			//其他数据正常采集
			count_capture++;    			 //触发采集次数增加（无效湿度数据不计采集次数）
			GetRtc(rtc_capture);       //读取当前时间，只记录时和分
			if(humidity > humidity_max) humidity_max = humidity; //计算最大湿度
			humidity_sum += humidity;  //湿度数据总和
			humidity_ave = humidity_sum * 1.0 / count_capture;//平均湿度=湿度总和/采集次数
			
			if(tem_capture > tem_max) tem_max = tem_capture; //计算最大温度
			tem_sum += tem_capture;  //温度总和
			tem_ave = tem_sum * 1.0 / count_capture;//平均温度=温度总和/采集次数
			
			//本次采集完成存储温度、湿度的旧值
			tem_capture_old = tem_capture;
			humidity_old = humidity;
		}
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
	ET1 = 1;				//使能定时器1中断
	EA = 1;
}

void Timer1_Isr(void) interrupt 3
{
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 100) SegSlow = 0;
	if(++TemSlow == 160) TemSlow = 0;
	if(++RtcSlow == 200) RtcSlow = 0;
	if(++AdSlow == 160) AdSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	if(++Time_1s == 1000)
	{
		Time_1s = 0;
		freq = (TH0 << 8) | TL0; //读取频率          
		TH0 = TL0 = 0;
	}
	//采集3秒退回原页面
	if(record)
	{
		if(++Time_3s == 3000)
		{
			Time_3s = 0;
			record = 0;
			SegMode = 0;
			has_record = 0;
		}
	}
	//长按S9两秒
	if(KeyPress)
	{
		if(++Time_2s >= 2000)
			Time_2s = 2001;//卡住时间
	}
	//指示灯闪烁
	if(tem_flag)
	{
		if(++Time_100ms == 100)
		{
			Time_100ms = 0;
			L4Flag = !L4Flag;
		}
	}
	else
	{
		Time_100ms = 0;
		L4Flag = 0;
	}
}

void main()
{
	SystemInit();
	TemRead();//温度空读 
	AdRead(); //AD空读
	SetRtc(Rtc);
	Timer0_Init();
	Timer1_Init();
	while(1)
	{
		LedProc();
		KeyProc();
		SegProc();
		TemProc();
		RtcProc();
		AdProc();
	}
}