#include "uart.h"
#include "stdio.h"

void Uart1_Init(void)	//9600bps@12.000MHz
{
	SCON = 0x50;		//8位数据,可变波特率
	AUXR &= 0xBF;		//定时器时钟12T模式
	AUXR &= 0xFE;		//串口1选择定时器1为波特率发生器
	TMOD &= 0x0F;		//设置定时器模式
	TL1 = 0xE6;			//设置定时初始值
	TH1 = 0xFF;			//设置定时初始值
	ET1 = 0;			//禁止定时器中断
	TR1 = 1;			//定时器1开始计时
	ES = 1;				//使能串口1中断
}

extern char putchar(char ch) {
	SBUF = ch;
	while (TI == 0);
	_nop_(); _nop_(); _nop_(); _nop_(); _nop_(); _nop_(); _nop_(); _nop_();
	TI = 0;
	return ch;
}