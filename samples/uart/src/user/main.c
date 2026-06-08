#include "uart.h"
#include "init.h"
#include "utils.h"
#include "seg.h"
#include "string.h"
#include "stdio.h"

idata u8 sg_sd = 0;
idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};

idata uint uart1_idx = 0;
idata uint uart1_tick = 0;
#define UART1_BUF_SIZE 30
pdata u8 uart1_buf[UART1_BUF_SIZE] = {
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
};

idata uint uart2_idx = 0;
idata uint uart2_tick = 0;
#define UART2_BUF_SIZE 30
pdata u8 uart2_buf[UART2_BUF_SIZE] = {
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
};
pdata u8 uart2_buf_9th[UART2_BUF_SIZE];

idata uint val = 0;
idata uint cnt = 0;

void sg_proc() {
	if (sg_sd < 100) return;
	sg_sd = 0;
	sg_buf[0] = cnt / 100 % 10;
	sg_buf[1] = cnt / 10 % 10;
	sg_buf[2] = cnt / 1 % 10;
	sg_buf[2] += ',';

	sg_buf[3] = val / 10000 % 10;
	sg_buf[4] = val / 1000 % 10;
	sg_buf[5] = val / 100 % 10;
	sg_buf[6] = val / 10 % 10;
	sg_buf[7] = val % 10;
}

void uart1_proc() {
	uint n, i;
	if (uart1_idx == 0) return;
	if (uart1_tick < 10) return;
	
	if (1 == sscanf(uart1_buf, "%u%n", &i, &n) && n == uart1_idx) {
		val = i;
		printf("%u", val + 1);
	} else {
		printf("Error");
	}
	
	memset(uart1_buf, 0, UART1_BUF_SIZE);
	uart1_idx = 0;
}

void uart2_proc() {
	int i;
	if (uart2_idx == 0) return;
	if (uart2_tick < 10) return;

	for (i = (int)uart2_idx - 1; i >= 0; i--) {
		Uart2_Send9Bit(uart2_buf[i], uart2_buf_9th[i]);
	}

	memset(uart2_buf, 0, UART2_BUF_SIZE);
	memset(uart2_buf_9th, 0, UART2_BUF_SIZE);
	uart2_idx = 0;
}

void Uart2_Isr(void) interrupt 8
{
	if (S2CON & S2RI)
	{
		S2CON &= ~S2RI;
		// store 9th bit (S2RB8)
		uart2_buf_9th[uart2_idx] = (S2CON & S2RB8) ? 1 : 0;

		uart2_buf[uart2_idx++] = S2BUF;

		if (uart2_idx >= UART2_BUF_SIZE) uart2_idx = 0;
		uart2_tick = 0;
	}
}


void Uart1_Isr(void) interrupt 4
{
	if (TI) {
		++cnt;
	}
	if (RI)				//检测串口1接收中断
	{
		uart1_buf[uart1_idx++] = SBUF;
		uart1_tick = 0;
		RI = 0;			//清除串口1接收中断请求位
	}
}

void Timer0_Isr(void) interrupt 1
{
	++sg_sd;
	++uart1_tick;
	++uart2_tick;
	if (++sg_pos == 8) sg_pos = 0;
	if (sg_buf[sg_pos] >= ',') {
		sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
	} else {
		sg_disp(sg_pos, sg_buf[sg_pos], 0);
	}
}

void Timer0_Init(void)		//1毫秒@12.000MHz
{
	AUXR &= 0x7F;			//定时器时钟12T模式
	TMOD &= 0xF0;			//设置定时器模式
	TL0 = 0x18;				//设置定时初始值
	TH0 = 0xFC;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
	ET0 = 1;				//使能定时器0中断
	EA = 1;
}

void main() {
	sys_init();
	Timer0_Init();
	Uart1_Init();
	Uart2_Init_115200_9Bit();
	while (1) {
		sg_proc();
		uart1_proc();
		uart2_proc();
	}
}