#include <STC15F2K60S2.H>
#include <init.h>
#include <key.h>
#include <seg.h>
#include <led.h>
#include <wave.h>
#include <uart.h>
#include <iic.h>
#include <ds18b20.h>
#include <string.h>
#include <stdio.h>

typedef unsigned char u8;
typedef unsigned int u16;
typedef unsigned long int u32;

/*延迟变量*/
idata u8 KeySlow;
idata u8 SegSlow;
idata u8 WaveSlow;
idata u8 TemSlow;
/*按键*/
idata u8 KeyVal,KeyDown,KeyUp,KeyOld;
idata u16 Time_1s_S12;      //S12长按计时变量
idata bit KeyPressS12;      //S12按下标志位
idata u16 Time_1s_S13;      //S13长按计时变量
idata bit KeyPressS13;      //S13按下标志位
/*数码管*/
idata u8 SegPos;
pdata u8 SegBuf[8] = {10,10,10,10,10,10,10,10};
pdata u8 SegPoint[8] = {0,0,0,0,0,0,0,0};
idata bit MainMode; 				//主页面 0-数据页面 1-参数页面
idata u8 SegMode;   				//数据页面分页面 0-温度数据 1-距离数据 2-变更次数
idata bit SetMode;  				//参数页面分页面 0-温度参数 1-距离参数
/*LED*/
pdata u8 ucLed[8] = {0,0,0,0,0,0,0,0};
/*温度&超声波*/
idata u16 tem_100x;					//温度放大100倍
idata u8 distance;  				//超声波测距结果
/*串口*/
pdata u8 uart_buf[10] = {0,0,0,0,0,0,0,0,0,0};
idata u8 uart_buf_index;
idata u8 uart_tick;
idata bit uart_flag;
/*其他变量*/
idata u16 set_change;          //参数改变次数
pdata u8 date_set[2] = {30,35};//参数存放数组 0-温度参数 1-距离参数
pdata u8 date_set_ctl[2];      //进入参数设置页面时存放旧值，用于比较参数是否发生变化
idata bit dac_flag = 1;        //DAC输出功能，上电启动

void KeyProc()
{
	if(KeySlow) return;
	KeySlow = 1;
	KeyVal = KeyDisp();
	KeyDown = KeyVal & ~KeyOld;
	KeyUp = ~KeyVal & KeyOld;
	KeyOld = KeyVal;
	
	//S12按下标志位有效并且计时变量清零
	if(KeyDown == 12)
	{
		Time_1s_S12 = 0;
		KeyPressS12 = 1;
	}
	if(Time_1s_S12 >= 1000)//长按
	{
		set_change = 0;
		EepromWrite(&set_change,0,2);
		KeyPressS12 = 0;
		Time_1s_S12 = 0;
	}
	if(KeyUp == 12)//松手检测短按
	{
		if(KeyPressS12 && Time_1s_S12 < 1000)//短按
		{
			if(!MainMode)
			{
				if(++SegMode == 3)
					SegMode = 0;
			}
			else
				SetMode = !SetMode;
		}
		KeyPressS12 = Time_1s_S12 = 0;//清空变量
	}
	//S13按下标志位有效并且计时变量清零
	if(KeyDown == 13)
	{
		Time_1s_S13 = 0;
		KeyPressS13 = 1;
	}
	if(Time_1s_S13 >= 1000)//长按
	{
		dac_flag = !dac_flag;
		KeyPressS13 = 0;
		Time_1s_S13 = 0;
	}
	if(KeyUp == 13)//松手检测短按
	{
		if(KeyPressS13 && Time_1s_S13 < 1000)
		{
			if(!MainMode)
			{
				SetMode = 0;//进入参数分页面默认温度参数
				date_set_ctl[0] = date_set[0];
				date_set_ctl[1] = date_set[1];
				MainMode = 1;
			}
			else
			{
				SegMode = 0;//进入数据分页面默认温度数据
				
				//如果参数变化了
				if(date_set[0] != date_set_ctl[0] || date_set[1] != date_set_ctl[1])
				{
					if(++set_change == 65536)//参数改变次数++
						set_change = 65535;
					EepromWrite(&set_change,0,2);//写入EEPROM的地址0和1
				}
				MainMode = 0;
			}
		}
		KeyPressS12 = Time_1s_S12 = 0;//清空变量
	}
	
	switch(KeyDown)
	{
		case 16://减
			if(MainMode)//参数页面
			{
				if(!SetMode)//并且处于温度参数页面，温度参数可调整范围：0~99
				{
					date_set[0] -= 2;
					if(date_set[0] >= 250)
						date_set[0] = 0;
				}
				else//距离参数分页面，距离参数可调整范围：0~99
				{
					date_set[1] -= 5;
					if(date_set[1] >= 250)
						date_set[1] = 0;
				}
			}
		break;
			
		case 17://加
			if(MainMode)//参数页面
			{
				if(!SetMode)//并且处于温度参数页面，温度参数可调整范围：0~99
				{
					date_set[0] += 2;
					if(date_set[0] >= 99)
						date_set[0] = 99;
				}
				else//距离参数分页面，距离参数可调整范围：0~99
				{
					date_set[1] += 5;
					if(date_set[1] >= 99)
						date_set[1] = 99;
				}
			}
		break;
	}
}

void SegProc()
{
	if(SegSlow) return;
	SegSlow = 1;
	if(!MainMode)//显示页面
	{
		switch(SegMode)
		{
			case 0://温度数据显示
				SegBuf[0] = 12;//C
				SegBuf[3] = 10;
				SegBuf[4] = tem_100x / 1000;
				SegBuf[5] = tem_100x / 100 % 10;
				SegPoint[5] = 1;
				SegBuf[6] = tem_100x / 10 % 10;
				SegBuf[7] = tem_100x % 10;
			break;
			
			case 1://距离数据显示
				SegBuf[0] = 13;//L
				SegBuf[4] = 10;
				SegBuf[5] = 10;
				SegPoint[5] = 0;
				SegBuf[6] = (distance / 10) ? distance / 10 : 10;
				SegBuf[7] = distance % 10;
			break;
			
			case 2://变更次数显示
				SegBuf[0] = 14;//q
				SegBuf[3] = (set_change/10000) ? set_change/10000 : 10;
			SegBuf[4] = (set_change/1000%10==0 && SegBuf[3]==10) ? 10 : set_change/1000%10;
			SegBuf[5] = (set_change/100%10==0 && SegBuf[4]==10) ? 10 : set_change/100%10;
			SegBuf[6] = (set_change/10%10==0 && SegBuf[5]==10) ? 10 : set_change/10%10;
				SegBuf[7] = set_change % 10;
			break;
		}
	}
	else//参数页面
	{
		SegPoint[5] = 0;
		SegBuf[0] = 15;//P
		SegBuf[3] = !SetMode ? 1 : 2;
		SegBuf[4] = 10;
		SegBuf[5] = 10;
		SegBuf[6] = (date_set[SetMode] / 10) ? date_set[SetMode] / 10 : 10;
		SegBuf[7] = date_set[SetMode] % 10;
	}
}

void LedProc()
{
	/*DAC*/
	if(dac_flag)
	{
		if(distance > date_set[1])
			DaWrite(2*51);
		else
			DaWrite(4*51);
	}
	else
		DaWrite(0.4*51);
	/*LED*/
	ucLed[0] = (tem_100x > (u16)(date_set[0]*100));
	ucLed[1] = (distance < date_set[1]);
	ucLed[2] = dac_flag;
}

void WaveProc()
{
	if(WaveSlow) return;
	WaveSlow = 1;
	distance = Wave() + 2;
}

void TemProc()
{
	if(TemSlow) return;
	TemSlow = 1;
	tem_100x = TemRead() * 100;
}

void UartProc()
{
	if(!uart_buf_index)
		return;
	if(uart_tick >= 10)
	{
		uart_flag = uart_tick = 0;
		/*数据解析*/
		if(memcmp(uart_buf,"ST\r\n",4) == 0)
			printf("$%bu,%.2f\r\n",distance,(float)tem_100x/100.0);
		else if(memcmp(uart_buf,"PARA\r\n",6) == 0)
			printf("#%bu,%bu\r\n",date_set[0],date_set[1]);
		else
			printf("ERROR\r\n");
		/*清除串口缓存区*/
		memset(uart_buf,0,uart_buf_index);
		uart_buf_index = 0;
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
	ET0 = 1;				//使能定时器0中断
	EA = 1;
}

void Timer0_Isr(void) interrupt 1
{
	if(uart_flag) uart_tick++;
	if(++KeySlow == 10) KeySlow = 0;
	if(++SegSlow == 10) SegSlow = 0;
	if(++WaveSlow == 10) WaveSlow = 0;
	if(++TemSlow == 10) TemSlow = 0;
	if(++SegPos == 8) SegPos = 0;
	SegDisp(SegPos,SegBuf[SegPos],SegPoint[SegPos]);
	LedDisp(ucLed);
	//按键长按
	if(KeyPressS12)
	{
		if(++Time_1s_S12 >= 1000)
			Time_1s_S12 = 1001;
	}
	if(KeyPressS13)
	{
		if(++Time_1s_S13 >= 1000)
			Time_1s_S13 = 1001;
	}
}

void Uart1_Isr(void) interrupt 4
{
	if(RI)
	{
		uart_flag = 1;
		uart_tick = 0;
		uart_buf[uart_buf_index++] = SBUF;
		RI = 0;
	}
	if(uart_buf_index > 10)
	{
		uart_flag = uart_tick = 0;
		uart_buf_index = 0;
		memset(uart_buf,0,10);
		
	}
}

void main()
{
	unsigned char eeprom_arr[2];
	SystemInit();
	TemRead();
	EepromRead(eeprom_arr,0,2);
	set_change = eeprom_arr[0] << 8 | eeprom_arr[1];//上电读取EEPROM
	Uart1_Init();
	Timer0_Init();
	while(1)
	{
		LedProc();
		KeyProc();
		SegProc();
		WaveProc();
		TemProc();
		UartProc();
	}
}