#include "eeprom.h"
#include "utils.h"

u8 t = 0xA5;

void Timer1_Isr(void) interrupt 3
{
	write0(0x80, t);
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


void main() {
	Timer1_Init();
	eeprom_write(0, &t, 1);
	t = 0;
	eeprom_read(0, &t, 1);
	while (1) {}
}