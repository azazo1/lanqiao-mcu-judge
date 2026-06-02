#include "utils.h"
#include "ds18b20.h"
#include "ds1302.h"
#include "init.h"
#include "key.h"
#include "pcf8591.h"
#include "seg.h"
#include "uart.h"
#include "led.h"
#include "stdio.h"
#include "string.h"
#include "us.h"

idata uint key_val, key_down, key_old, key_up;

#define is_down(x) ((key_down >> (x - 4)) & 1)
#define is_up(x) ((key_up >> (x - 4)) & 1)
#define is_pressing(x) ((key_val >> (x - 4)) & 1)

#define UART_BUF_SIZE 100
pdata u8 uart_buf[UART_BUF_SIZE] = {
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
};
idata u8 uart_idx = 0;
idata u8 uart_tick = 0;
#define UART_TICK_MAX 100

idata u8 key_sd;
idata u8 sg_sd;
idata u8 sg_pos = 0;
pdata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};

#define MENU_DATA 0
#define MENU_PARAM 1
#define MENU_STD 2
idata u8 menu = MENU_DATA;

#define DATA_TIME 0
#define DATA_LIQ 1
#define DATA_VOL 2
#define DATA_WEIGHT 3
idata u8 data_mode = DATA_TIME;

#define PARAM_H1 0
#define PARAM_H2 1
#define PARAM_F 2
#define PARAM_S 3
#define PARAM_R 4
#define PARAM_L 5
#define PARAM_W 6
#define PARAM_H 7
idata u8 param = PARAM_H1;

#define S_TYPE_RECT 2
#define S_TYPE_ROUND 1
#define S_TYPE_CIRCLE 0

#define STD_0 0
#define STD_5 1
#define STD_10 2
#define STD_15 3
#define STD_20 4
idata u8 std = STD_0;

idata uint h1_param_10x = 10;
idata uint h2_param_10x = 1;
idata uint freq_param = 2000;
idata u8 s_param = S_TYPE_RECT;
idata uint r_param_10x = 10;
idata uint l_param_10x = 10;
idata uint h_param_10x = 10;
idata uint w_param_10x = 10;
pdata u8 rtc_buf[3] = {23, 59, 50};
pdata u8 volt_std_10x[5] = {0, 10, 20, 40, 50};
pdata u8 volt_std_10x_alt[5] = {0, 10, 20, 40, 50};
#define volt_std_lvl(s) (volt_std_10x[(s)] * 255.0f / 50.0f)

#define VOLT_STD_10x_UPPER 50
#define VOLT_STD_10x_LOWER 0

idata u8 weight_10x = 0;
idata int tempe = 0;
idata int tempe_abs = 0;
idata uint liq_height_100x = 0;
idata u8 vol_spare_ratio = 0;
idata uint vol_liq_10x = 0;
idata uint dist = 0;

idata uint error = 0; // when not 0, has error
bit error_isr_502 = 0;
bit error_isr_503 = 0;
bit error_isr_504 = 0;
idata long error_isr_val_504;
idata uint error_isr_val_503;
idata uint error_isr_val_502;

idata u8 fk7_tick = 0;
#define FK7_TIME 100

bit fk8 = 0;
idata u8 fk8_tick = 0;
#define FK8_TIME 200
idata uint fk8_elapsed = 0;
#define FK8_DURATION 3000

idata uint freq = 0;
#define FREQ_TIME 1000
idata uint freq_tick = 0;
#define FREQ_QUEUE_SIZE 5
pdata uint freq_queue[FREQ_QUEUE_SIZE] = {0, 0, 0, 0, 0};
idata u8 freq_queue_idx = 0;
idata u8 freq_queue_idx_initial = 0;

void led_proc() {
	led_buf[0] = menu == MENU_DATA && data_mode == DATA_TIME;
	led_buf[1] = menu == MENU_DATA && data_mode == DATA_LIQ;
	led_buf[2] = menu == MENU_DATA && data_mode == DATA_VOL;
	led_buf[3] = menu == MENU_DATA && data_mode == DATA_WEIGHT;
	
	led_buf[4] = error == 500 || error == 501;
	led_buf[5] = error_isr_502;
	if (!error_isr_503) {
		led_buf[6] = 0;
	}
	if (!error_isr_504 && !fk8) {
		led_buf[7] = 0;
	}
}

void key_proc() {
	u8 i, valid;
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & ( key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		if (menu == MENU_DATA) {
			menu = MENU_PARAM;
			param = PARAM_H1;
		} else if (menu == MENU_PARAM) {
			menu = MENU_STD;
			std = STD_0;
			for (i = 0; i < 5; ++i) volt_std_10x_alt[i] = volt_std_10x[i];
		} else if (menu == MENU_STD) {
			menu = MENU_DATA;
			data_mode = DATA_TIME;
			valid = 1;
			for (i = 0; i < 5; ++i) {
				if (volt_std_10x_alt[i] >= VOLT_STD_10x_LOWER
					&& volt_std_10x_alt[i] <= VOLT_STD_10x_UPPER
					&& (i == 0 || volt_std_10x_alt[i] > volt_std_10x_alt[i - 1])) {
				} else {
					valid = 0;
					break;
				}
			}
			if (valid)
				for (i = 0; i < 5; ++i) volt_std_10x[i] = volt_std_10x_alt[i];
		}
	}
	
	if (is_down(5)) {
		if (menu == MENU_DATA) {
			if (++data_mode > DATA_WEIGHT) data_mode = DATA_TIME;
		} else if (menu == MENU_PARAM) {
			if (++param > PARAM_H) param = PARAM_H1;
		} else if (menu == MENU_STD) {
			if (++std > STD_20) std = STD_0;
		}
	}
	
	if (is_down(8) && menu == MENU_STD) {
		if (volt_std_10x_alt[std] < 99)
			volt_std_10x_alt[std] += 1;
	}
	if (is_down(9) && menu == MENU_STD) {
		if (volt_std_10x_alt[std] > 0) {
			volt_std_10x_alt[std] -= 1;
		}
	}
}

void sg_proc() {
	u8 i;
	uint lvl;
	float tf;
	if (sg_sd < 80) return;
	sg_sd = 0;
	
	read_rtc(rtc_buf);
	tempe = read_t();
	tempe_abs = tempe > 0 ? tempe : -tempe;

	dist = us_dist(tempe * 0.6 + 330);
	if (s_param == S_TYPE_CIRCLE) {
		if (2 * r_param_10x * 10 > dist){
			liq_height_100x = 2 * r_param_10x * 10 - dist;
		} else {
			liq_height_100x = 0;
		}
		vol_liq_10x = 3.14 * (
			-1.0 * liq_height_100x / 10.0 * liq_height_100x / 10.0 * liq_height_100x / 10.0 / 3
			+ r_param_10x * liq_height_100x / 10.0 * liq_height_100x / 10.0
		) / 100;
		// printf("%f %f", -1.0 * liq_height_100x / 10.0 * liq_height_100x / 10.0 * liq_height_100x / 10.0 / 3, + r_param_10x * liq_height_100x / 10.0 * liq_height_100x / 10.0);
		tf = 100.0 * vol_liq_10x / 10.0 / (4.0 / 3 * 3.14 * r_param_10x / 10.0 * r_param_10x / 10.0 * r_param_10x / 10.0);
	} else if (s_param == S_TYPE_ROUND) {
		if (h_param_10x * 10 > dist) {
			liq_height_100x = h_param_10x * 10 - dist;
		} else {
			liq_height_100x = 0;
		}
		vol_liq_10x = 3.14 * 2 * r_param_10x * liq_height_100x / 100.0;
		tf = 100.0 * liq_height_100x / 10 / h_param_10x;
	} else if (s_param == S_TYPE_RECT) {
		if (h_param_10x * 10 > dist) {
			liq_height_100x = h_param_10x * 10 - dist;
		} else {
			liq_height_100x = 0;
		}
		vol_liq_10x = 10.0 * w_param_10x / 10.0 * l_param_10x / 10.0 * liq_height_100x / 100.0;
		tf = 100.0 * liq_height_100x / 10 / h_param_10x;
	}
	if (tf > 100) tf = 100;
	if (tf < 0) tf = 0;
	vol_spare_ratio = 100 - tf;

	if (liq_height_100x > h1_param_10x * 10) {
		if (error != 500)
			printf("[%u,%02bu%02bu%02bu,%.2f]", 500, rtc_buf[0], rtc_buf[1], rtc_buf[2], liq_height_100x / 100.0);
		error = 500;
	} else if (liq_height_100x < h2_param_10x * 10) {
		if (error != 501)
			printf("[%u,%02bu%02bu%02bu,%.2f]", 501, rtc_buf[0], rtc_buf[1], rtc_buf[2], liq_height_100x / 100.0);
		error = 501;
	} else {
		error = 0;
	}
	lvl = adc(0x43) + (0.04 * tempe * tempe - 10 * tempe + 225) / 1000 * 51;
	// printf("%u, %bu, %bu, %bu, %bu, %bu", lvl, lvl <= volt_std_lvl(STD_0), lvl <= volt_std_lvl(STD_5), lvl <= volt_std_lvl(STD_10), lvl <= volt_std_lvl(STD_15), lvl <= volt_std_lvl(STD_20));
	if (lvl <= volt_std_lvl(STD_0)) {
		weight_10x = 0;
	} else if (lvl <= volt_std_lvl(STD_5)) {
		weight_10x = 10 * (0 + (lvl - volt_std_lvl(STD_0)) * 5 / (volt_std_lvl(STD_5) - volt_std_lvl(STD_0)));
	} else if (lvl <= volt_std_lvl(STD_10)) {
		weight_10x = 10 * (5 + (lvl - volt_std_lvl(STD_5)) * 5 / (volt_std_lvl(STD_10) - volt_std_lvl(STD_5)));
	} else if (lvl <= volt_std_lvl(STD_15)) {
		weight_10x = 10 * (10 + (lvl - volt_std_lvl(STD_10)) * 5 / (volt_std_lvl(STD_15) - volt_std_lvl(STD_10)));
	} else if (lvl <= volt_std_lvl(STD_20)) {
		weight_10x = 10 * (15 + (lvl - volt_std_lvl(STD_15)) * 5 / (volt_std_lvl(STD_20) - volt_std_lvl(STD_15)));
	} else {
		weight_10x = 200;
	}
	/* printf("%u, %u, %u, %u, %u",
		freq_queue[(freq_queue_idx + FREQ_QUEUE_SIZE - 4) % FREQ_QUEUE_SIZE],
		freq_queue[(freq_queue_idx + FREQ_QUEUE_SIZE - 3) % FREQ_QUEUE_SIZE],
		freq_queue[(freq_queue_idx + FREQ_QUEUE_SIZE - 2) % FREQ_QUEUE_SIZE],
		freq_queue[(freq_queue_idx + FREQ_QUEUE_SIZE - 1) % FREQ_QUEUE_SIZE],
		freq_queue[(freq_queue_idx + FREQ_QUEUE_SIZE) % FREQ_QUEUE_SIZE]
	); // todo remove */

	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	switch (menu) {
		case MENU_DATA:
			switch (data_mode) {
				case DATA_TIME:
					sg_buf[0] = rtc_buf[0] / 10;
					sg_buf[1] = rtc_buf[0] % 10;
					sg_buf[2] = 11; // -
					sg_buf[3] = rtc_buf[1] / 10;
					sg_buf[4] = rtc_buf[1] % 10;
					sg_buf[5] = 11; // -
					sg_buf[6] = rtc_buf[2] / 10;
					sg_buf[7] = rtc_buf[2] % 10;
					break;
				case DATA_LIQ:
					if (tempe >= 0) {
						sg_buf[1] = tempe >= 100 ? tempe / 100 % 10 : 10;
						sg_buf[2] = tempe >= 10 ? tempe / 10 % 10 : 10;
						sg_buf[3] = tempe % 10;
					} else {
						if (tempe_abs >= 100) {
						} else if (tempe_abs >= 10) {
							sg_buf[1] = 11; // -
							sg_buf[2] = tempe_abs / 10 % 10;
							sg_buf[3] = tempe_abs % 10;
						} else {
							sg_buf[2] = 11; // -
							sg_buf[3] = tempe_abs % 10;
						}
					}
					sg_buf[4] = 11; // -
					sg_buf[5] = (liq_height_100x / 100 % 10) + ',';
					sg_buf[6] = liq_height_100x / 10 % 10;
					sg_buf[7] = liq_height_100x / 1 % 10;
					break;
				case DATA_VOL:
					sg_buf[0] = vol_spare_ratio >= 100 ? vol_spare_ratio / 100 % 10 : 10;
					sg_buf[1] = vol_spare_ratio >= 10 ? vol_spare_ratio / 10 % 10 : 10;
					sg_buf[2] = vol_spare_ratio % 10;
					sg_buf[3] = 11; // -
					sg_buf[4] = vol_liq_10x >= 1000 ? vol_liq_10x / 1000 % 10 : 10;
					sg_buf[5] = vol_liq_10x >= 100 ? vol_liq_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (vol_liq_10x / 10 % 10);
					sg_buf[7] = vol_liq_10x % 10;
					break;
				case DATA_WEIGHT:
					if (tempe >= 0) {
						sg_buf[0] = tempe >= 100 ? tempe / 100 % 10 : 10;
						sg_buf[1] = tempe >= 10 ? tempe / 10 % 10 : 10;
						sg_buf[2] = tempe % 10;
					} else {
						if (tempe_abs >= 100) {
						} else if (tempe_abs >= 10) {
							sg_buf[0] = 11; // -
							sg_buf[1] = tempe_abs / 10 % 10;
							sg_buf[2] = tempe_abs % 10;
						} else {
							sg_buf[1] = 11; // -
							sg_buf[2] = tempe_abs % 10;
						}
					}
					sg_buf[3] = 11; // -
					sg_buf[4] = weight_10x >= 1000 ? weight_10x / 1000 % 10 : 10;
					sg_buf[5] = weight_10x >= 100 ? weight_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (weight_10x / 10 % 10);
					sg_buf[7] = weight_10x % 10;
					break;
				default:;
			}
			break;
		case MENU_PARAM:
			sg_buf[0] = 12; // P
			sg_buf[2] = 11; // -
			switch (param) {
				case PARAM_H1:
					sg_buf[1] = 1;
					sg_buf[3] = h1_param_10x >= 10000 ? h1_param_10x / 10000 % 10 : 10;
					sg_buf[4] = h1_param_10x >= 1000 ? h1_param_10x / 1000 % 10 : 10;
					sg_buf[5] = h1_param_10x >= 100 ? h1_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (h1_param_10x / 10 % 10);
					sg_buf[7] = h1_param_10x % 10;
					break;
				case PARAM_H2:
					sg_buf[1] = 2;
					sg_buf[3] = h2_param_10x >= 10000 ? h2_param_10x / 10000 % 10 : 10;
					sg_buf[4] = h2_param_10x >= 1000 ? h2_param_10x / 1000 % 10 : 10;
					sg_buf[5] = h2_param_10x >= 100 ? h2_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (h2_param_10x / 10 % 10);
					sg_buf[7] = h2_param_10x % 10;
					break;
				case PARAM_F:
					sg_buf[1] = 3;
					sg_buf[3] = freq_param >= 10000 ? freq_param / 10000 % 10 : 10;
					sg_buf[4] = freq_param >= 1000 ? freq_param / 1000 % 10 : 10;
					sg_buf[5] = freq_param >= 100 ? freq_param / 100 % 10 : 10;
					sg_buf[6] = freq_param >= 10 ? freq_param / 10 % 10 : 10;
					sg_buf[7] = freq_param % 10;
					break;
				case PARAM_S:
					sg_buf[1] = 4;
					sg_buf[7] = s_param;
					break;
				case PARAM_R:
					sg_buf[1] = 5;
					sg_buf[3] = r_param_10x >= 10000 ? r_param_10x / 10000 % 10 : 10;
					sg_buf[4] = r_param_10x >= 1000 ? r_param_10x / 1000 % 10 : 10;
					sg_buf[5] = r_param_10x >= 100 ? r_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (r_param_10x / 10 % 10);
					sg_buf[7] = r_param_10x % 10;
					break;
				case PARAM_L:
					sg_buf[1] = 6;
					sg_buf[3] = l_param_10x >= 10000 ? l_param_10x / 10000 % 10 : 10;
					sg_buf[4] = l_param_10x >= 1000 ? l_param_10x / 1000 % 10 : 10;
					sg_buf[5] = l_param_10x >= 100 ? l_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (l_param_10x / 10 % 10);
					sg_buf[7] = l_param_10x % 10;
					break;
				case PARAM_W:
					sg_buf[1] = 7;
					sg_buf[3] = w_param_10x >= 10000 ? w_param_10x / 10000 % 10 : 10;
					sg_buf[4] = w_param_10x >= 1000 ? w_param_10x / 1000 % 10 : 10;
					sg_buf[5] = w_param_10x >= 100 ? w_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (w_param_10x / 10 % 10);
					sg_buf[7] = w_param_10x % 10;
					break;
				case PARAM_H:
					sg_buf[1] = 8;
					sg_buf[3] = h_param_10x >= 10000 ? h_param_10x / 10000 % 10 : 10;
					sg_buf[4] = h_param_10x >= 1000 ? h_param_10x / 1000 % 10 : 10;
					sg_buf[5] = h_param_10x >= 100 ? h_param_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (h_param_10x / 10 % 10);
					sg_buf[7] = h_param_10x % 10;
					break;
			}
			break;
		case MENU_STD:
			sg_buf[0] = 13; // E
			switch (std) {
				case STD_0:
					sg_buf[1] = 0;
					sg_buf[2] = 0;
					break;
				case STD_5:
					sg_buf[1] = 0;
					sg_buf[2] = 5;
					break;
				case STD_10:
					sg_buf[1] = 1;
					sg_buf[2] = 0;
					break;
				case STD_15:
					sg_buf[1] = 1;
					sg_buf[2] = 5;
					break;
				case STD_20:
					sg_buf[1] = 2;
					sg_buf[2] = 0;
					break;
			}
			sg_buf[6] = ',' + (volt_std_10x_alt[std] / 10 % 10);
			sg_buf[7] = volt_std_10x_alt[std] % 10;
			break;
		default:;
	}
}

// return index found, else returns end (>= uart_idx)
u8 find(u8 start, char ch) {
	u8 i;
	for (i = start; i < uart_idx; ++i) {
		if (ch == uart_buf[i]) {
			break;
		}
	}
	return i;
}

void uart_proc() {
	if (uart_idx == 0) return;
	if (uart_tick < 10) return;
	
	// printf("%s", uart_buf);
	if (strchr(uart_buf, '?')) { // query, strchr returns not 0 when finds
		u8 idx = find(0, '(');
		idx += 1;
		switch (uart_buf[idx]) {
			case 'H':
				if (uart_buf[idx + 1] == ',') { // PARAM_H
					printf("(H,%.1f)", h_param_10x / 10.0);
				} else if (uart_buf[idx + 1] == '1') { // PARAM_H1
					printf("(H1,%.1f)", h1_param_10x / 10.0);
				} else if (uart_buf[idx + 1] == '2') { // PARAM_H2
					printf("(H2,%.1f)", h2_param_10x / 10.0);
				} else {
					printf("Error: invalid param type H?"); // debug
				}
				break;
			case 'F':
				printf("(F,%u)", freq_param);
				break;
			case 'S':
				printf("(S,%bu)", s_param); // u8
				break;
			case 'r':
				printf("(r,%.1f)", r_param_10x / 10.0);
				break;
			case 'L':
				printf("(L,%.1f)", l_param_10x / 10.0);
				break;
			case 'W':
				printf("(W,%.1f)", w_param_10x / 10.0);
				break;
			default:
				printf("Error: invalid param type: %c", uart_buf[idx]); // debug
		} // end switch
	} else if (strchr(uart_buf, ':')) {
		unsigned long tl;
		u8 h, m, s;
		if (1 == sscanf(uart_buf + 3, "%lu", &tl)) {
			h = tl / 10000 % 100;
			m = tl / 100 % 100;
			s = tl % 100;
			if (h <= 23 && m <= 59 && s <= 59) {
				rtc_buf[0] = h;
				rtc_buf[1] = m;
				rtc_buf[2] = s;
				set_rtc(rtc_buf);
				printf("OK");
			} else {
				printf("ERROR");
			}
			// printf("%lu %lu %lu %s", tl / 10000 % 100, tl / 100 % 100, tl % 100, uart_buf + 3);
		} else {
			printf("Error: failed to get time format: %s", uart_buf + 3);
		}
	} else { // param config
		u8 idx = find(0, '('); // (H,xxx)(H1,xx)(F,xx) find '('
		u8 has_warn = 0;
		u8 has_ok = 0;
		while (idx < uart_idx) {
			float tf;
			uint tu;
			idx += 1;
			switch (uart_buf[idx]) {
				case 'H':
					if (uart_buf[idx + 1] == ',') { // PARAM_H
						idx += 2;
						if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
							h_param_10x = tf * 10;
							has_ok = 1;
						} else {
							printf("Error: failed to get param H value"); // debug
						}
					} else if (uart_buf[idx + 1] == '1') { // PARAM_H1
						idx += 3;
						if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
							h1_param_10x = tf * 10;
							has_ok = 1;
						} else {
							printf("Error: failed to get param H1 value"); // debug
						}
					} else if (uart_buf[idx + 1] == '2') { // PARAM_H2
						idx += 3;
						if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
							h2_param_10x = tf * 10;
							has_ok = 1;
						} else {
							printf("Error: failed to get param H2 value"); // debug
						}
					} else {
						printf("Error: invalid param type H?"); // debug
					}
					break;
				case 'F':
					idx += 2;
					if (1 == sscanf(uart_buf + idx, "%u", &tu)) {
						freq_param = tu;
						has_ok = 1;
					} else {
						printf("Error: failed to get param F value"); // debug
					}
					break;
				case 'S':
					idx += 2;
					if (1 == sscanf(uart_buf + idx, "%u", &tu)) {
						if (tu > 2) {
							has_warn = 1;
						} else {
							s_param = tu;
							has_ok = 1;
						}
					} else {
						printf("Error: failed to get param S value"); // debug
					}
					break;
				case 'r':
					idx += 2;
					if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
						r_param_10x = tf * 10;
						has_ok = 1;
					} else {
						printf("Error: failed to get param r value"); // debug
					}
					break;
				case 'L':
					idx += 2;
					if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
						l_param_10x = tf * 10;
						has_ok = 1;
					} else {
						printf("Error: failed to get param L value"); // debug
					}
					break;
				case 'W':
					idx += 2;
					if (1 == sscanf(uart_buf + idx, "%f", &tf)) {
						w_param_10x = tf * 10;
						has_ok = 1;
					} else {
						printf("Error: failed to get param W value");
					}
					break;
				default:
					printf("Error: invalid param type: %c", uart_buf[idx]); // debug
			}
			idx = find(idx, '(');
		}
		if (has_ok && has_warn) {
			printf("WARN");
		} else if (has_ok) {
			printf("OK");
		} else {
			printf("ERROR");
		}
	}

	// when error, send []
	
	uart_idx = 0;
	memset(uart_buf, 0, UART_BUF_SIZE);
}

void Uart1_Isr(void) interrupt 4
{
	if (RI)				//检测串口1接收中断
	{
		uart_tick = 0;
		uart_buf[uart_idx++] = SBUF;
		RI = 0;			//清除串口1接收中断请求位
	}
}

void timer1_isr() interrupt 3 {
	key_sd++;
	sg_sd++;
	if (uart_tick < UART_TICK_MAX) uart_tick++;

	if (++freq_tick == FREQ_TIME) {
		freq_tick = 0;
		freq = (TH0 << 8 | TL0) * (1000.0 / FREQ_TIME);
		TH0 = TL0 = 0;
		
		if (++freq_queue_idx == FREQ_QUEUE_SIZE) {
			freq_queue_idx = 0;
		}
		if (freq_queue_idx_initial < FREQ_QUEUE_SIZE) {
			freq_queue_idx_initial++;
		}
				
		freq_queue[freq_queue_idx % FREQ_QUEUE_SIZE] = freq;
		
		if (freq > freq_param) {
			error_isr_502 = 1;
			error_isr_val_502 = freq;
		} else {
			error_isr_502 = 0;
		}
		if (freq_queue_idx_initial >= 5) {
			u8 i;
			u8 descend = 1;
			u8 ascend = 1;
			for (i = freq_queue_idx + FREQ_QUEUE_SIZE - 4; i < freq_queue_idx + FREQ_QUEUE_SIZE; ++i) {
				if (freq_queue[i % FREQ_QUEUE_SIZE] > freq_queue[(i + 1) % FREQ_QUEUE_SIZE]) {
					ascend = 0;
				} else if (freq_queue[i % FREQ_QUEUE_SIZE] < freq_queue[(i + 1) % FREQ_QUEUE_SIZE]) {
					descend = 0;
				} else {
					ascend = 0;
					descend = 0;
					break;
				}
			}
			if (ascend || descend) {
				error_isr_503 = 1;
				error_isr_val_503 = descend ? 0 : 1;
			} else {
				error_isr_503 = 0;
			}
		}
		if (freq_queue_idx_initial >= 2) {
			u8 i;
			long diff;
			/* uint fmax = 0, fmin = 50000;
			for (
				i = freq_queue_idx + FREQ_QUEUE_SIZE - 2;
				i <= freq_queue_idx + FREQ_QUEUE_SIZE;
				++i
			) {
				if (freq_queue[i % FREQ_QUEUE_SIZE] < fmin) {
					fmin = freq_queue[i % FREQ_QUEUE_SIZE];
				} else if (freq_queue[i % FREQ_QUEUE_SIZE] > fmax) {
					fmax = freq_queue[i % FREQ_QUEUE_SIZE];
				}
			} */
			i = freq_queue_idx + FREQ_QUEUE_SIZE - 1;
			diff = (long)freq_queue[(i + 1) % FREQ_QUEUE_SIZE] - (long)freq_queue[i % FREQ_QUEUE_SIZE];
			// if (fmax > fmin + 1000) {
			if (diff < -1000 || diff > 1000) {
				error_isr_504 = 1;
				error_isr_val_504 = diff;
				fk8_elapsed = 0;
				fk8 = 1;
			} else {
				error_isr_504 = 0;
			}
		}
	}
	
	if (++sg_pos == 8) sg_pos = 0;
	if (sg_buf[sg_pos] >= ',') {
		sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
	} else {
		sg_disp(sg_pos, sg_buf[sg_pos], 0);
	}
	
	if (error_isr_503) {
		if (++fk7_tick == FK7_TIME) {
			fk7_tick = 0;
			led_buf[6] = !led_buf[6];
		}
	}

	if (fk8) {
		if (fk8_elapsed < FK8_DURATION) fk8_elapsed++;
		else fk8 = 0;

		if (++fk8_tick == FK8_TIME) {
			fk8_tick = 0;
			led_buf[7] = !led_buf[7];
		}
	}
	
	relay(error_isr_504 || error_isr_503 || error_isr_502 || error);
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

void Timer0_Init(void)		// ne555
{
	TMOD &= 0xF0;			//设置定时器模式
	TMOD |= 0x05;			//设置定时器模式
	TL0 = 0x00;				//设置定时初始值
	TH0 = 0x0;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
}

bit last_error_isr_504 = 0;
bit last_error_isr_503 = 0;
bit last_error_isr_502 = 0;
void error_proc() {
	if (last_error_isr_504 == error_isr_504 && last_error_isr_503 == error_isr_503 && last_error_isr_502 == error_isr_502) return;

	if (error_isr_502 && !last_error_isr_502) {
		printf("[%u,%02bu%02bu%02bu,%u]", 502, rtc_buf[0], rtc_buf[1], rtc_buf[2], error_isr_val_502);
	} else if (error_isr_503 && !last_error_isr_503) {
		printf("[%u,%02bu%02bu%02bu,%u]", 503, rtc_buf[0], rtc_buf[1], rtc_buf[2], error_isr_val_503);
	} else if (error_isr_504 && !last_error_isr_504) {
		printf("[%u,%02bu%02bu%02bu,%ld]", 504, rtc_buf[0], rtc_buf[1], rtc_buf[2], error_isr_val_504);
	}

	last_error_isr_504 = error_isr_504;
	last_error_isr_503 = error_isr_503;
	last_error_isr_502 = error_isr_502;
}

void main() {
	sys_init();
	Timer0_Init();
	Timer1_Init();
	Uart1_Init();

	set_rtc(rtc_buf);

	while (1) {
		key_proc();
		sg_proc();
		uart_proc();
		led_proc();
		error_proc();
	}
}