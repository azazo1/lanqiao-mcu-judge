#include "utils.h"
#include "key.h"
#include "seg.h"
#include "led.h"
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

// 1kHz led pwm signal
// duty can be adjust by S8/S9
idata uint led_duty = 0;
idata uint led_cur = 0;
#define LED_PERIOD 10

void key_proc() {
	if (key_sd < 10) return;
	key_sd = 0;
	
	key_old = key_val;
	key_val = key_read();
	key_down = key_val & (key_val ^ key_old);
	key_up = ~key_val & (key_val ^ key_old);
	
	if (is_down(8)) {
		if (led_duty > 0) led_duty--;
	}
	if (is_down(9)) {
		if (led_duty < LED_PERIOD) led_duty++;
	}
}

void sg_proc() {
	u8 i;
	if (sg_sd < 100) return;
	sg_sd = 0;
	
	for (i = 0; i < 8; ++i) sg_buf[i] = 10;
	sg_buf[0] = led_duty / 100 % 10;
	sg_buf[1] = led_duty / 10 % 10;
	sg_buf[2] = led_duty % 10;
}

void Timer0_Isr(void) interrupt 1 {
	if (++led_cur == LED_PERIOD) led_cur = 0;
	if (led_cur <= led_duty) {
		led_buf[0] = 1;
	} else {
		led_buf[0] = 0;
	}

	led_disp(led_buf);
}

void Timer0_Init(void)		//100微秒@12.000MHz
{
	AUXR &= 0x7F;			//定时器时钟12T模式
	TMOD &= 0xF0;			//设置定时器模式
	TL0 = 0x9C;				//设置定时初始值
	TH0 = 0xFF;				//设置定时初始值
	TF0 = 0;				//清除TF0标志
	TR0 = 1;				//定时器0开始计时
	ET0 = 1;				//使能定时器0中断
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
	Timer0_Init();
	Timer1_Init();
	while (1) {
		key_proc();
		sg_proc();
	}
}