#include "ds1302.h"
#include "seg.h"
#include "key.h"
#include "init.h"
#include "utils.h"

idata u8 key_sd = 0;
idata u8 sg_sd = 0;
idata uint key_val, key_old, key_up, key_down;

#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_pressing >> ((x) - 4)) & 1)

pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata u8 sg_pos = 0;

pdata u8 time_buf[3] = {13, 59, 58};
pdata u8 date_buf[4] = {26, 6, 5, 5};

pdata u8 time_buf_alt[3] = {23, 59, 58};
pdata u8 date_buf_alt[4] = {26, 6, 5, 5};

bit setting = 0;
bit is_date = 0; // 0 set/show time, 1 set/show date
idata u8 set_idx = 0;
// is_date == 0: 0 set hour, 1 set minute, 2 set second
// is_date == 1: 0 set year, 1 set month, 2 set date, 3 set day

bit fk = 0; // selected setting item flicker
idata uint fk_tick = 0;
#define FK_TIME 500

u8 rtc_get_rem() {
	u8 rem = 1;
	if (is_date) {
		switch (set_idx) {
			case 0: // year
				rem = 100;
				break;
			case 1: // month
				rem = 13;
				break;
			case 2: // date
				rem = 32;
				break;
			case 3: // day
				rem = 8;
				break;
			default:;
		}
	} else {
		switch (set_idx) {
			case 0: // hour
				rem = 24;
				break;
			case 1: // minute
				rem = 60;
				break;
			case 2: // second
				rem = 60;
				break;
			default:;
		}
	}
	return rem;
}

void key_proc() {
	u8 i;
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) { // toggle setting; when quit setting, save; when setting, time read pause.
		if (!setting) {
			setting = 1;
			set_idx = 0; // set_idx = 0 and keep is_date
			for (i = 0; i < 4; ++i) date_buf_alt[i] = date_buf[i];
			for (i = 0; i < 3; ++i) time_buf_alt[i] = time_buf[i];
		} else {
			setting = 0;
			// todo check date validity, if not pass, no write.
			// 1. ymd, day: 1..x, no 0
			// 2. Feb date 28 / 29
			// 3. date 30 / 31
			for (i = 0; i < 4; ++i) date_buf[i] = date_buf_alt[i];
			for (i = 0; i < 3; ++i) time_buf[i] = time_buf_alt[i];
			rtc_set_time(time_buf);
			rtc_set_date(date_buf);
		}
	}
	
	if (is_down(5)) { // toggle is_date
		is_date = !is_date;
		if (setting) {
			set_idx = 0;
		}
	}
	
	if (is_down(6)) { // down set_idx
		if (setting) {
			set_idx = (set_idx + (is_date ? 4 : 3) - 1) % (is_date ? 4 : 3);
		}
	}
	
	if (is_down(7)) { // up set_idx
		if (setting) {
			set_idx = (set_idx + 1) % (is_date ? 4 : 3);
		}
	}
	
	if (is_down(8)) { // down set val, wrapping
		if (setting) {
			if (is_date) {
				u8 rem = rtc_get_rem();
				if (date_buf_alt[set_idx] == 1) {
					date_buf_alt[set_idx] = rem - 1;
				} else {
					date_buf_alt[set_idx] -= 1;
				}
			} else {
				u8 rem = rtc_get_rem();
				if (time_buf_alt[set_idx] == 1) {
					time_buf_alt[set_idx] = rem - 1;
				} else {
					time_buf_alt[set_idx] -= 1;
				}
			}
		}
	}
	
	if (is_down(9)) { // up set val, wrapping
		if (setting) {
			if (is_date) {
				u8 rem = rtc_get_rem();
				if (date_buf_alt[set_idx] == rem - 1) {
					date_buf_alt[set_idx] = 1;
				} else {
					date_buf_alt[set_idx]++;
				}
			} else {
				u8 rem = rtc_get_rem();
				if (time_buf_alt[set_idx] == rem - 1) {
					time_buf_alt[set_idx] = 1;
				} else {
					time_buf_alt[set_idx]++;
				}
			}
		}
	}
	
	if (is_down(12)) { // toggle is12
		rtc_set_12h(!rtc_get_12h()); // rtc_set_time only supports 24h writing, just setting to 12-23 to write pm.
	}
}

void sg_proc() {
	u8 i;
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	rtc_get_time(time_buf);
	rtc_get_date(date_buf);
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	if (setting) {
		if (is_date) {
			sg_buf[0] = date_buf_alt[0] / 10 % 10;
			sg_buf[1] = date_buf_alt[0] % 10;
			sg_buf[1] += ',';
			sg_buf[2] = date_buf_alt[1] / 10 % 10;
			sg_buf[3] = date_buf_alt[1] % 10;
			sg_buf[3] += ',';
			sg_buf[4] = date_buf_alt[2] / 10 % 10;
			sg_buf[5] = date_buf_alt[2] % 10;
			sg_buf[5] += ',';
			sg_buf[6] = date_buf_alt[3] / 10 % 10;
			sg_buf[7] = date_buf_alt[3] % 10;
			if (!fk) {
				sg_buf[set_idx * 2] = sg_buf[set_idx * 2 + 1] =10;
			}
		} else {
			sg_buf[2] = time_buf_alt[0] / 10 % 10;
			sg_buf[3] = time_buf_alt[0] % 10;
			sg_buf[3] += ',';
			sg_buf[4] = time_buf_alt[1] / 10 % 10;
			sg_buf[5] = time_buf_alt[1] % 10;
			sg_buf[5] += ',';
			sg_buf[6] = time_buf_alt[2] / 10 % 10;
			sg_buf[7] = time_buf_alt[2] % 10;
			if (!fk) {
				sg_buf[2 + set_idx * 2] = sg_buf[2 + set_idx * 2 + 1] =10;
			}
		}
	} else {
		if (is_date) {
			sg_buf[0] = date_buf[0] / 10 % 10;
			sg_buf[1] = date_buf[0] % 10;
			sg_buf[1] += ',';
			sg_buf[2] = date_buf[1] / 10 % 10;
			sg_buf[3] = date_buf[1] % 10;
			sg_buf[3] += ',';
			sg_buf[4] = date_buf[2] / 10 % 10;
			sg_buf[5] = date_buf[2] % 10;
			sg_buf[5] += ',';
			sg_buf[6] = date_buf[3] / 10 % 10;
			sg_buf[7] = date_buf[3] % 10;
		} else {
			sg_buf[2] = time_buf[0] / 10 % 10;
			sg_buf[3] = time_buf[0] % 10;
			sg_buf[3] += ',';
			sg_buf[4] = time_buf[1] / 10 % 10;
			sg_buf[5] = time_buf[1] % 10;
			sg_buf[5] += ',';
			sg_buf[6] = time_buf[2] / 10 % 10;
			sg_buf[7] = time_buf[2] % 10;
		}
	}
}

void Timer1_Isr(void) interrupt 3 {
	++sg_sd;
	++key_sd;
	
	if (setting) {
		if (++fk_tick == FK_TIME) {
			fk_tick = 0;
			fk = !fk;
		}
	}

	if (++sg_pos == 8) sg_pos = 0;
	if (sg_buf[sg_pos] >= ',') {
		sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
	} else {
		sg_disp(sg_pos, sg_buf[sg_pos], 0);
	}
}

void Timer1_Init(void)		//1毫秒@12.000MHz
{
	AUXR |= 0x40;			//定时器时钟1T模式
	TMOD &= 0x0F;			//设置定时器模式
	TL1 = 0x20;				//设置定时初始值
	TH1 = 0xD1;				//设置定时初始值
	TF1 = 0;				//清除TF1标志
	TR1 = 1;				//定时器1开始计时
	ET1 = 1;				//使能定时器1中断
	EA = 1;
}


void main() {
	sys_init();
	rtc_set_date(date_buf);
	rtc_set_time(time_buf);

	Timer1_Init();
	while (1) {
		sg_proc();
		key_proc();
	}
}