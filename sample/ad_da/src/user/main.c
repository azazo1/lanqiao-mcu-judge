#include "init.h"
#include "key.h"
#include "pcf8591.h"
#include "seg.h"
#include "utils.h"
#include <STC15F2K60S2.H>

idata u8 sg_sd;
#define SG_SD 20
idata u8 key_sd;
#define KEY_SD 10
idata u8 ad_da_sd;
#define AD_DA_SD 150

idata u8 sg_pos;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};

idata u8 key_val, key_old, key_down, key_up;

idata u8 output_lvl = 127;
idata u8 input_volt, brightness;

bit detect_rb = 1;

void timer1_init(void) // 1毫秒@12.000MHz
{
  AUXR &= 0xBF; // 定时器时钟12T模式
  TMOD &= 0x0F; // 设置定时器模式
  TL1 = 0x18;   // 设置定时初始值
  TH1 = 0xFC;   // 设置定时初始值
  TF1 = 0;      // 清除TF1标志
  TR1 = 1;      // 定时器1开始计时
  ET1 = 1;
  EA = 1;
}

void timer1_isr() interrupt 3 {
  ++sg_sd;
  ++key_sd;
  ++ad_da_sd;

  if (++sg_pos == 8)
    sg_pos = 0;
  if (sg_buf[sg_pos] >= ',')
    sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
  else
    sg_disp(sg_pos, sg_buf[sg_pos], 0);
}

void key_proc() {
  if (key_sd < KEY_SD)
    return;
  key_sd = 0;

  key_val = key_read();
  key_down = key_val & (key_val ^ key_old);
  key_up = ~key_val & (key_val ^ key_old);
  key_old = key_val;

  switch (key_down) {
  case 6:
    --output_lvl;
    break;
  case 7:
    ++output_lvl;
    break;
  default:;
  }
}

void ad_da_proc() {
  if (ad_da_sd < AD_DA_SD)
    return;
  ad_da_sd = 0;

		input_volt = adc(0x43);
		brightness = adc(0x41);

	detect_rb = !detect_rb;
	
  dac(output_lvl);
}

void sg_proc() {
  if (sg_sd < SG_SD)
    return;
  sg_sd = 0;

  sg_buf[0] = input_volt / 100 % 10;
  sg_buf[1] = input_volt / 10 % 10;
  sg_buf[2] = input_volt % 10;
  sg_buf[4] = brightness / 100 % 10;
  sg_buf[5] = brightness / 10 % 10;
  sg_buf[6] = brightness % 10;
}

void main() {
  sys_init();
  timer1_init();
  while (1) {
    sg_proc();
    key_proc();
    ad_da_proc();
  }
}