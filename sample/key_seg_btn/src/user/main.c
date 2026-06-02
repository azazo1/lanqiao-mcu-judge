#include "led.h"
#include "utils.h"
#include "init.h"
#include "seg.h"
#include "key.h"

idata uint key_val, key_down, key_up, key_old;

#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_pressing >> ((x) - 4)) & 1)

idata u8 key_sd = 0;
idata u8 sg_sd = 0;
pdata u8 led_buf[8] = {0,0,0,0,0,0,0,0};
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata u8 sg_pos = 0;

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		led_buf[0] = !led_buf[0];
	}
	if (is_down(5)) {
		led_buf[1] = !led_buf[1];
	}
	if (is_down(6)) {
		led_buf[2] = !led_buf[2];
	}
	if (is_down(7)) {
		led_buf[3] = !led_buf[3];
	}
}

void sg_proc() {
	if (sg_sd < 100) return;
	sg_sd = 0;
	sg_buf[3] = key_val >= 10000 ? key_val / 10000 % 10 : 10;
	sg_buf[4] = key_val >= 1000 ? key_val / 1000 % 10 : 10;
	sg_buf[5] = key_val >= 100 ? key_val / 100 % 10 : 10;
	sg_buf[6] = key_val >= 10 ? key_val / 10 % 10 : 10;
	sg_buf[7] = key_val % 10;
}

void timer1_isr() interrupt 3 {
	key_sd++;
	sg_sd++;
	
	if (++sg_pos == 8) sg_pos = 0;
	if (sg_buf[sg_pos] >= ',') {
		sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
	} else {
		sg_disp(sg_pos, sg_buf[sg_pos], 0);
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
		key_proc();
		sg_proc();
	}
}