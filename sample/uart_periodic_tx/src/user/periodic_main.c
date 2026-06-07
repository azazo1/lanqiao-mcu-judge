#include "init.h"
#include "seg.h"
#include "uart.h"
#include "stdio.h"

#define SEND_START_MS 120
#define SEND_INTERVAL_MS 280

// 120 400 680 960

idata u8 sg_pos = 0;
idata u8 sg_tick = 0;
pdata u8 sg_buf[8] = {0, 0, 10, 10, 10, 10, 10, 10};

idata uint uptime_ms = SEND_INTERVAL_MS;
idata uint initial_ms = 0;
idata uint send_count = 0;

void sg_proc() {
	if (sg_tick < 100) return;
	sg_tick = 0;
	sg_buf[0] = send_count / 10 % 10;
	sg_buf[1] = send_count % 10;
}

void send_frame() {
	if (send_count & 0x01) {
		printf("ERROR");
	} else {
		printf("OK");
	}
	++send_count;
}

void uart_proc() {
	if (initial_ms < SEND_START_MS) return;
	if (uptime_ms < SEND_INTERVAL_MS) return;

	uptime_ms -= SEND_INTERVAL_MS;
	send_frame();
}

void Timer0_Isr(void) interrupt 1
{
	++sg_tick;
	if (initial_ms < SEND_INTERVAL_MS) {
		initial_ms++;
	} else {
		++uptime_ms;
	}
	if (++sg_pos == 8) sg_pos = 0;
	sg_disp(sg_pos, sg_buf[sg_pos], 0);
}

void Timer0_Init(void)
{
	AUXR &= 0x7F;
	TMOD &= 0xF0;
	TL0 = 0x18;
	TH0 = 0xFC;
	TF0 = 0;
	TR0 = 1;
	ET0 = 1;
	EA = 1;
}

void main() {
	sys_init();
	Timer0_Init();
	Uart1_Init();
	ES = 0;
	while (1) {
		sg_proc();
		uart_proc();
	}
}
