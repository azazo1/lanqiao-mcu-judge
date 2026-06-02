#include "utils.h"
#include "init.h"
#include "us.h"
#include "seg.h"
#include "key.h"

idata u8 key_sd = 0;
idata u8 sg_sd = 0;
idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata uint key_val, key_old, key_up, key_down;
#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_val >> ((x) - 4)) & 1)

#define MENU_DIST 0
#define MENU_SPEED 1
idata u8 menu = MENU_DIST;

idata uint speed = 340;
idata uint dist = 0;

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		if (menu == MENU_DIST) {
			menu = MENU_SPEED;
		} else if (menu == MENU_SPEED) {
			menu = MENU_DIST;
		}
	}
	
	if (is_down(8)) {
		if (speed > 300) {
			speed -= 5;
		}
	}
	if (is_down(9)) {
		if (speed < 400) {
			speed += 5;
		}
	}
}

void sg_proc() {
	u8 i;
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	dist = us_dist(speed);
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;

	if (menu == MENU_DIST) {
		sg_buf[0] = 11; // L
		sg_buf[3] = dist >= 10000 ? dist / 10000 % 10 : 10;
		sg_buf[4] = dist >= 1000 ? dist / 1000 % 10 : 10;
		sg_buf[5] = dist >= 100 ? dist / 100 % 10 : 10;
		sg_buf[6] = dist >= 10 ? dist / 10 % 10 : 10;
		sg_buf[7] = dist % 10;
	} else if (menu == MENU_SPEED) {
		sg_buf[0] = 12; // P
		sg_buf[5] = speed >= 100 ? speed / 100 % 10 : 10;
		sg_buf[6] = speed >= 10 ? speed / 10 % 10 : 10;
		sg_buf[7] = speed % 10;
	}
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