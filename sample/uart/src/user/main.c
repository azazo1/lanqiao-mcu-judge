#include "uart.h"
#include "init.h"
#include "utils.h"
#include "seg.h"
#include "string.h"
#include "stdio.h"

idata u8 sg_sd = 0;
idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};

idata uint uart_idx = 0;
idata uint uart_tick = 0;
#define UART_BUF_SIZE 30
pdata u8 uart_buf[UART_BUF_SIZE] = {
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
};

idata uint val = 0;

void sg_proc() {
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	sg_buf[3] = val / 10000 % 10;
	sg_buf[4] = val / 1000 % 10;
	sg_buf[5] = val / 100 % 10;
	sg_buf[6] = val / 10 % 10;
	sg_buf[7] = val % 10;
}

void uart_proc() {
	uint n, i;
	if (uart_idx == 0) return;
	if (uart_tick < 10) return;
	
	if (1 == sscanf(uart_buf, "%u%n", &i, &n) && n == uart_idx) {
		val = i;
		printf("%u", val + 1);
	} else {
		printf("Error");
	}
	
	memset(uart_buf, 0, UART_BUF_SIZE);
	uart_idx = 0;
}

void Uart1_Isr(void) interrupt 4
{
	if (RI)				//检测串口1接收中断
	{
		uart_buf[uart_idx++] = SBUF;
		uart_tick = 0;
		RI = 0;			//清除串口1接收中断请求位
	}
}

void Timer1_Isr(void) interrupt 3
{
	++sg_sd;
	++uart_tick;
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
	ET1 = 1;				//使能定时器1中断
	EA = 1;
}

void main() {
	sys_init();
	Timer1_Init();
	Uart1_Init();
	while (1) {
		sg_proc();
		uart_proc();
	}
}