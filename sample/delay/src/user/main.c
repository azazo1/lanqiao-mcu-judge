#include <STC15F2K60S2.H>
#define u8 unsigned char
#include "intrins.h"

void write0(u8 pos, u8 val) {
	P0 = val;
	P2 = P2 & 0x1f | pos;
	P2 &= 0x1f;
}

void Delay5ms(void)	//@12.000MHz
{
	unsigned char data i, j;

	i = 59;
	j = 90;
	do
	{
		while (--j);
	} while (--i);
}

void main() {
	u8 t = 1;
	write0(0x80, 0xff);
	while (1) {
		Delay5ms();
		if (!t) {
			t = 1;
		}
		write0(0x80, ~t);
		t <<= 1;
	}
}