#include "led.h"

static idata u8 led_old = 0;
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
		led_old = led;
		write0(0x80, ~led);
	}
}