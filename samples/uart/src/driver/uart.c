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

void Uart2_Init(void)	//9600bps@12.000MHz
{
	S2CON = 0x50;		//8位数据,可变波特率
	AUXR &= 0xFB;		//定时器时钟12T模式
	T2L = 0xE6;			//设置定时初始值
	T2H = 0xFF;			//设置定时初始值
	AUXR |= 0x10;		//定时器2开始计时
	IE2 |= 0x01;		//使能串口2中断
}

void Uart2_Init_115200_9Bit(void)	//115200bps@12.000MHz
{
	S2CON = 0xD0;		//9位数据,可变波特率
	AUXR |= 0x04;		//定时器时钟1T模式
	T2L = 0xE6;			//设置定时初始值
	T2H = 0xFF;			//设置定时初始值
	AUXR |= 0x10;		//定时器2开始计时
	IE2 |= 0x01;		//使能串口2中断
}

void Uart2_Send9Bit(u8 dat, bit b9) {
    if (b9) S2CON |= S2TB8;  // 设置第9位为1
    else    S2CON &= ~S2TB8; // 设置第9位为0
    
    S2BUF = dat;
    while (!(S2CON & S2TI)); // 等待发送完成
    S2CON &= ~S2TI;          // 清除标志
}

extern char putchar(char ch) {
	SBUF = ch;
	while (TI == 0);
	TI = 0;
	return ch;
}