#include "led.h"
#include "utils.h"
#include "init.h"

idata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};

idata u8 fk1_tick = 0;
#define FK1_TIME 100

void timer1_isr() interrupt 3 {
	if (++fk1_tick == FK1_TIME) {
		fk1_tick = 0;
		led_buf[0] = !led_buf[0];
	}
	led_disp(led_buf);
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

void main() {
	sys_init();
	Timer1_Init();
	while (1) {
	}
}