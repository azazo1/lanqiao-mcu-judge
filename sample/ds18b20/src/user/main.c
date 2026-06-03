#include "utils.h"
#include "key.h"
#include "seg.h"
#include "led.h"
#include "ds18b20.h"
#include "init.h"

idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata u8 key_sd = 0;
idata u8 sg_sd = 0;
pdata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};

idata uint key_val, key_old, key_down, key_up;
#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_val >> ((x) - 4)) & 1)

idata long tempe_1000x = 0;
idata u8 tempe_resolution_level = 0;

idata uint initial_tick = 0;
#define INITIAL_TIME 500

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		if (tempe_resolution_level > 0) {
			tempe_resolution_level--;
		} else {
			tempe_resolution_level = 3;
		}
		tempe_set_resolution(tempe_resolution_level);
	}
	
	if (is_down(5)) {
		if (tempe_resolution_level < 3) {
			tempe_resolution_level++;
		} else {
			tempe_resolution_level = 0;
		}
		tempe_set_resolution(tempe_resolution_level);
	}
}

// need to test:
// each tempe resolution 0~3
// tempe range -25~100

void sg_proc() {
	u8 i;
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	tempe_1000x = tempe_get() * 1000;
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	
	sg_buf[0] = tempe_1000x >= 100000 ? tempe_1000x / 100000 % 10 : 10;
	sg_buf[1] = tempe_1000x >= 10000 ? tempe_1000x / 10000 % 10 : 10;
	sg_buf[2] = ',' + (tempe_1000x / 1000 % 10);
	sg_buf[3] = tempe_1000x / 100 % 10;
	sg_buf[4] = tempe_1000x / 10 % 10;
	sg_buf[5] = tempe_1000x % 10;
	
	sg_buf[7] = tempe_resolution_level;
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
	
	if (initial_tick < INITIAL_TIME) {
		initial_tick++;
		led_buf[0] = 1;
	} else {
		led_buf[0] = 0;
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
	tempe_set_resolution(tempe_resolution_level);
	while (1) {
		key_proc();
		sg_proc();
	}
}