#include "utils.h"
#include "init.h"
#include "us.h"
#include "key.h"
#include "led.h"
#include "seg.h"
#include "pcf8591.h"
#include "uart.h"
#include "stdio.h"
#include "string.h"
#include "math.h"

idata u8 key_sd = 0;
idata u8 sg_sd = 0;
idata u8 physics_sd = 0;
#define PHYSICS_SD 50

pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
idata u8 sg_pos = 0;
pdata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};

idata uint key_val, key_down, key_up, key_old;
#define is_down(x) ((key_down >> (x - 4)) & 1)
#define is_up(x) ((key_up >> (x - 4)) & 1)
#define is_pressing(x) ((key_val >> (x - 4)) & 1)

idata u8 uart_idx = 0;
#define UART_BUF_SIZE 50
pdata u8 uart_buf[UART_BUF_SIZE] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
idata u8 uart_tick = 0;

#define STATUS_IDLE 0
#define STATUS_WAIT 1
#define STATUS_RUN 2
idata u8 status = STATUS_IDLE;
// speed 0 (IDLE), accept position, click S4 -> RUN
// RUN to target -> IDLE
// RUN with button -> WAIT
// RUN with obstacle -> WAIT
// WAIT with button -> RUN

// device X, Y
idata int x = 0, y = 0;
idata float fx = 0, fy = 0; // 当前坐标的浮点表示
// target X, Y
idata int tx = 0, ty = 0;

bit has_target = 0; // set 0 when arrive
	
idata uint freq_tick = 0;
#define FREQ_TIME 300 // todo 不知道为什么频率只能测到 30k
idata uint freq = 0;
idata uint v_10x = 0;

idata uint dist = 0;
#define obstacle (dist < 30)
bit sun = 0;

idata u8 r_param_10x = 10;
idata int b_param = 0;
idata u8 r_param_10x_alt = 10;
idata int b_param_alt = 0;
#define R_PARAM_10x_UPPER 20
#define R_PARAM_10x_LOWER 10
#define B_PARAM_UPPER 90
#define B_PARAM_LOWER -90

#define MENU_POS 0
#define MENU_VELO 1
#define MENU_PARAM 2
idata u8 menu = MENU_POS;

bit is_b_param = 0;

bit arrival = 0;
idata uint arrival_tick = 0;
#define ARRIVAL_TIME 3000

bit flicker = 0;
idata u8 flicker_tick = 0;
#define FLICKER_TIME 100

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(4)) {
		if (status == STATUS_IDLE && has_target) {
			status = STATUS_RUN;
		} else if (status == STATUS_RUN) {
			status = STATUS_WAIT;
		} else if (status == STATUS_WAIT && !obstacle) {
			status = STATUS_RUN;
		}
	}
	
	if (is_down(5) && status == STATUS_IDLE) {
		x = 0;
		y = 0;
		fx = 0;
		fy = 0;
	}
	
	if (is_down(8)) {
		if (menu == MENU_POS) {
			menu = MENU_VELO;
		} else if (menu == MENU_VELO) {
			menu = MENU_PARAM;
			is_b_param = 0;
			b_param_alt = b_param;
			r_param_10x_alt = r_param_10x;
		} else if (menu == MENU_PARAM) {
			menu = MENU_POS;
			b_param = b_param_alt;
			r_param_10x = r_param_10x_alt;
		}
	}
	
	if (is_down(9)) {
		is_b_param = !is_b_param;
	}
	
	if (is_down(12) && menu == MENU_PARAM) {
		if (is_b_param) {
			if (b_param_alt < B_PARAM_UPPER)
				b_param_alt += 5;
		} else {
			if (r_param_10x_alt < R_PARAM_10x_UPPER)
				r_param_10x_alt += 1;
		}
	}
	
	if (is_down(13) && menu == MENU_PARAM) {
		if (is_b_param) {
			if (b_param_alt > B_PARAM_LOWER)
				b_param_alt -= 5;
		} else {
			if (r_param_10x_alt > R_PARAM_10x_LOWER)
				r_param_10x_alt -= 1;
		}
	}
}

void sg_proc() {
	u8 i;
	u8 b_param_alt_abs = b_param_alt > 0 ? b_param_alt : -b_param_alt;
	if (sg_sd < 100) return;
	sg_sd = 0;

	dist = us_dist();
	if (obstacle) {
		if (status == STATUS_RUN) {
			status = STATUS_WAIT;
		}
	}
	sun = adc(0x41) > 61; // 1.2V <=> 61.2
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	switch (menu) {
		case MENU_POS:
			sg_buf[0] = 11; // L
			sg_buf[4] = 12; // -
			switch (status) {
				case STATUS_IDLE:
					sg_buf[1] = x >= 100 ? x / 100 % 10 : 10;
					sg_buf[2] = x >= 10 ? x / 10 % 10 : 10;
					sg_buf[3] = x % 10;
					sg_buf[5] = y >= 100 ? y / 100 % 10 : 10;
					sg_buf[6] = y >= 10 ? y / 10 % 10 : 10;
					sg_buf[7] = y % 10;
					break;
				case STATUS_WAIT:
				case STATUS_RUN:
					sg_buf[1] = tx >= 100 ? tx / 100 % 10 : 10;
					sg_buf[2] = tx >= 10 ? tx / 10 % 10 : 10;
					sg_buf[3] = tx % 10;
					sg_buf[5] = ty >= 100 ? ty / 100 % 10 : 10;
					sg_buf[6] = ty >= 10 ? ty / 10 % 10 : 10;
					sg_buf[7] = ty % 10;
					break;
				default:;
			}
			break;
		case MENU_VELO:
			sg_buf[0] = 13; // E
			switch (status) {
				case STATUS_RUN:
					sg_buf[1] = 1;
					sg_buf[3] = v_10x >= 10000 ? v_10x / 10000 % 10 : 10;
					sg_buf[4] = v_10x >= 1000 ? v_10x / 1000 % 10 : 10;
					sg_buf[5] = v_10x >= 100 ? v_10x / 100 % 10 : 10;
					sg_buf[6] = ',' + (v_10x >= 10 ? v_10x / 10 % 10 : 0);
					sg_buf[7] = v_10x % 10;
					break;
				case STATUS_IDLE:
					sg_buf[1] = 2;
					sg_buf[3] = sg_buf[4] = sg_buf[5] = sg_buf[6] = sg_buf[7] = 12; // -
					break;
				case STATUS_WAIT:
					sg_buf[1] = 3;
					sg_buf[3] = dist >= 10000 ? dist / 10000 % 10 : 10;
					sg_buf[4] = dist >= 1000 ? dist / 1000 % 10 : 10;
					sg_buf[5] = dist >= 100 ? dist / 100 % 10 : 10;
					sg_buf[6] = dist >= 10 ? dist / 10 % 10 : 10;
					sg_buf[7] = dist % 10;
					break;
				default:;
			}
			break;
		case MENU_PARAM:
			sg_buf[0] = 14;
			sg_buf[2] = ',' + (r_param_10x_alt / 10 % 10);
			sg_buf[3] = r_param_10x_alt % 10;
			if (b_param_alt < 0) {
				if (b_param_alt_abs >= 10) {
					sg_buf[5] = 12; // 12: -
					sg_buf[6] = b_param_alt_abs >= 10 ? b_param_alt_abs / 10 % 10 : 10;
					sg_buf[7] = b_param_alt_abs % 10;
				} else {
					sg_buf[6] = 12; // 12: -
					sg_buf[7] = b_param_alt_abs % 10;
				}
			} else {
				sg_buf[6] = b_param_alt >= 10 ? b_param_alt / 10 % 10 : 10;
				sg_buf[7] = b_param_alt % 10;
			}
			break;
		default:;
	}
}

void Uart1_Isr(void) interrupt 4
{
	if (RI)
	{
		uart_buf[uart_idx++] = SBUF;
		// if (uart_idx == UART_BUF_SIZE) uart_idx = 0;
		uart_tick = 0;
		RI = 0;
	}
}

void uart_proc() {
	int nx, ny, n;
	if (uart_idx == 0) return;
	if (uart_tick < 10) return;

	// printf("%s", uart_buf);
	// if (strcmp(uart_buf, "~") == 0) { // todo del
	//	printf("%u Hz", freq);
	// }
	if (strcmp(uart_buf, "?") == 0) {
		switch (status) {
			case STATUS_IDLE:
				printf("Idle");
				break;
			case STATUS_WAIT:
				printf("Wait");
				break;
			case STATUS_RUN:
				printf("Busy");
				break;
			default:
				printf("Invalid Status");
		}
	} else if (strcmp(uart_buf, "#") == 0) {
		printf("(%d,%d)", x, y);
	} else if (2 == sscanf(uart_buf, "(%d,%d)%n", &nx, &ny, &n) && n == strlen(uart_buf)) {
		if (status == STATUS_IDLE) {
			printf("Got it");
			tx = nx;
			ty = ny;
			// status = STATUS_RUN; // S4
			has_target = 1;
		} else {
			printf("Busy");
		}
	} else {
		printf("Error");
	}
	
	memset(uart_buf, 0, UART_BUF_SIZE);
	uart_idx = 0;
}

void timer1_isr() interrupt 3 {
	key_sd++;
	sg_sd++;
	uart_tick++;
	if (status == STATUS_RUN)
		physics_sd++;
	
	if (++freq_tick == FREQ_TIME) {
		freq_tick = 0;
		freq = (TH0 << 8 | TL0) * (1000.0 / FREQ_TIME);
		TH0 = TL0 = 0;
		v_10x = (3.14 / 100 * r_param_10x * freq) + 10 * b_param;
	}
	
	if (++sg_pos == 8) sg_pos = 0;
	if (sg_buf[sg_pos] >= ',') {
		sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
	} else {
		sg_disp(sg_pos, sg_buf[sg_pos], 0);
	}

	relay(status == STATUS_RUN);
	if (status == STATUS_IDLE) {
		led_buf[0] = 0;
	} else if (status == STATUS_RUN) {
		led_buf[0] = 1;
	} else if (status == STATUS_WAIT) {
		if (++flicker_tick == FLICKER_TIME) {
			flicker_tick = 0;
			flicker = !flicker;
		}
		led_buf[0] = flicker;
	}
	led_buf[1] = status == STATUS_RUN && !sun;
	if (++arrival_tick == ARRIVAL_TIME) {
		arrival_tick = 0;
		arrival = 0;
		led_buf[2] = 0;
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

void Timer0_Init(void) // ne555
{
	TMOD &= 0xF0;			//设置定时器模式
	TMOD |= 0x05;			//设置定时器模式
	TL0 = 0;				//设置定时初始值
	TH0 = 0;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
}

void physics_proc() {
	float dx = tx - fx;
	float dy = ty - fy;
	float rx = fx, ry = fy;
	float covered;
	float length;
	if (physics_sd < PHYSICS_SD) return;

	// velo calculation
	// set L3 on arrival, arrival = 1, arrival_tick = 0
	// set has_target = 0 on arrival
	// set status = STATUS_IDLE on arrival
	// advance only when STATUS_RUN
	if (status == STATUS_RUN) {
		covered = 0.0001 * v_10x * physics_sd;
		length = sqrt(dx * dx + dy * dy);
		fx += covered / length * dx;
		fy += covered / length * dy;
		if (
			(rx > fx ? (fx <= tx && tx <= rx) : (rx <= tx && tx <= fx))
			&& (ry > fy ? (fy <= ty && ty <= ry) : (ry <= ty && ty <= fy))
		) {
			fx = tx;
			fy = ty;
			arrival = 1;
			led_buf[2] = 1;
			arrival_tick = 0;
			has_target = 0;
			status = STATUS_IDLE;
		}
		x = fx;
		y = fy;
	}

	
	physics_sd = 0;
}

void main() {
	sys_init();
	Timer0_Init();
	Timer1_Init();
	Uart1_Init();
	while (1) {
		key_proc();
		sg_proc();
		uart_proc();
		physics_proc();
	}
}