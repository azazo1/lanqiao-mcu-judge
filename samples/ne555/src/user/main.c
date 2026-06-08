#include <STC15F2K60S2.H>
#include "seg.h"
#define u8 unsigned char
#define uint unsigned int

#define SG_SD 20
u8 sg_sd;

u8 sg_buf[] = {10, 10, 10, 10, 10, 10, 0, 10};
u8 sg_pos;

uint freq;
uint freq_tick;
#define FREQ_WINDOW 1000

u8 key_read() {
	u8 t = 0;
	P44 = 0; P42 = 1; P35 = 1;
	if (P33 == 0) t = 4;
	if (P32 == 0) t = 5;
	if (P31 == 0) t = 6;
	if (P30 == 0) t = 7;
	
	P44 = 1; P42 = 0; P35 = 1;
	if (P33 == 0) t = 8;
	if (P32 == 0) t = 9;
	if (P31 == 0) t = 10;
	if (P30 == 0) t = 11;
	
	P44 = 1; P42 = 1; P35 = 0;
	if (P33 == 0) t = 12;
	if (P32 == 0) t = 13;
	if (P31 == 0) t = 14;
	if (P30 == 0) t = 15;
	
	return t;
}

u8 key_sd = 0;
u8 key_down, key_up, key_val, key_old;
void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
}

void sys_init() {
	write_p0(0xff, 0x80);
	write_p0(0x00, 0xa0);
}

void timer0_init(void)		//1毫秒@12.000MHz
{
	TMOD &= 0xF0;			//设置定时器模式
	TMOD |= 0x05;			//设置定时器模式
	TL0 = 0;				//设置定时初始值
	TH0 = 0;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
}

void timer1_init(void)		//1毫秒@12.000MHz
{
	AUXR &= 0xBF;			//定时器时钟12T模式
	TMOD &= 0x0F;			//设置定时器模式
	TL1 = 0x18;				//设置定时初始值
	TH1 = 0xFC;				//设置定时初始值
	TF1 = 0;				//清除TF1标志
	TR1 = 1;				//定时器1开始计时
	EA = 1;
	ET1 = 1;
}

void timer1_isr() interrupt 3 {
	++sg_sd;
	++key_sd;
	if (++sg_pos == 8) sg_pos = 0;
	sg_disp(sg_pos, sg_buf[sg_pos], 0);
	
	if (++freq_tick == FREQ_WINDOW) {
		freq = TH0 << 8 | TL0;
		TH0 = TL0 = 0;
		freq_tick = 0;
	}
}

void sg_proc() {
	if (sg_sd < SG_SD) return;
	sg_sd = 0;
	
	sg_buf[0] = key_val / 100 % 10;
	sg_buf[1] = key_val / 10 % 10;
	sg_buf[2] = key_val % 10;
	
	sg_buf[3] = (freq > 10000) ? freq / 10000 % 10 : 10;
	sg_buf[4] = (freq > 1000) ? freq / 1000 % 10 : 10;
	sg_buf[5] = (freq > 100) ? freq / 100 % 10 : 10;
	sg_buf[6] = (freq > 10) ? freq / 10 % 10 : 10;
	sg_buf[7] = freq % 10;
}

void main() {
	sys_init();
	timer0_init();
	timer1_init();
	while (1) {
		key_proc();
		sg_proc();
	}
}