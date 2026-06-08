#include "utils.h"
#include "key.h"
#include "seg.h"
#include "led.h"
#include "init.h"
#include "eeprom.h"
#include "ds18b20.h"

idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata u8 key_sd = 0;
idata u8 sg_sd = 0;
pdata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};

idata uint key_val, key_old, key_down, key_up;
#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_val >> ((x) - 4)) & 1)

idata u8 cursor = 0x00;
#define CURSOR_UPPER 0x20
#define CURSOR_LOWER 0x00

idata u8 tempe_res = 0;
idata u8 cur_byte = 0;
idata u8 session_count = 0;

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		session_count = (session_count + 1) % 100;
	}
	
	if (is_down(5)) {
		tempe_res = (tempe_res + 3) % 4;
		tempe_set_res(tempe_res);
	}

	if (is_down(6)) {
		tempe_res = (tempe_res + 1) % 4;
		tempe_set_res(tempe_res);
	}
	
	if (is_down(7)) {
		if (cursor == CURSOR_LOWER) {
			cursor = CURSOR_UPPER;
		} else {
			cursor--;
		}
		eeprom_read(cursor, &cur_byte, 1);
	}
	
	if (is_down(8)) {
		if (cursor == CURSOR_UPPER) {
			cursor = CURSOR_LOWER;
		} else {
			cursor++;
		}
		eeprom_read(cursor, &cur_byte, 1);
	}
	
	if (is_down(9)) {
		cur_byte--;
		eeprom_write(cursor, &cur_byte, 1);
	}
	
	if (is_down(10)) {
		cur_byte++;
		eeprom_write(cursor, &cur_byte, 1);
	}
}

void sg_proc() {
	u8 i;
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	
	sg_buf[0] = session_count / 10 % 10;
	sg_buf[1] = session_count % 10;
	sg_buf[2] = 11; // -
	sg_buf[3] = tempe_res;
	sg_buf[4] = 11; // -
	sg_buf[5] = cur_byte / 100 % 10;
	sg_buf[6] = cur_byte / 10 % 10;
	sg_buf[7] = cur_byte % 10;
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

	tempe_res = tempe_get_res();
	eeprom_read(cursor, &cur_byte, 1);

	Timer1_Init();

	while (1) {
		key_proc();
		sg_proc();
	}
}