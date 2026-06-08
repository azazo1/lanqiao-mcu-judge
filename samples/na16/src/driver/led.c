#include "led.h"

static idata u8 led_old = 0x00;
void led_disp(u8 *buf) {
	u8 led = buf[0] << 0 |
	buf[1] << 1 |
	buf[2] << 2 |
	buf[3] << 3 |
	buf[4] << 4 |
	buf[5] << 5 |
	buf[6] << 6 |
	buf[7] << 7;
	
	if (led != led_old) {
		write0(0x80, ~led);
		led_old = led;
	}
}

static idata u8 peri_old = 0x00;

static void set_peri(u8 pos, u8 on) {
	u8 peri = peri_old;
	if (on) {
		peri |= pos;
	} else {
		peri &= ~pos;
	}
	
	if (peri != peri_old) {
		peri_old = peri;
		write0(0xa0, peri);
	}
}

void relay(bit on) {
	set_peri(0x10, on);
}