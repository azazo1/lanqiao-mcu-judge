#include "us.h"

sbit us_tx = P1^0;
sbit us_rx = P1^1;

void Delay10us(void)	//@12.000MHz
{
	unsigned char data i;

	_nop_();
	_nop_();
	i = 27;
	while (--i);
}

uint us_dist() {
	uint t;
	CMOD = 0;
	CL = CH = 0;
	us_tx = 1; Delay10us(); us_tx = 0;
	
	CR = 1;
	while (CF == 0 && us_rx == 1);
	CR = 0;
	
	if (CF) {
		CF = 0;
		return 0;
	} else {
		t = CH << 8 | CL;
		t = t * 0.017;
		return t;
	}
}