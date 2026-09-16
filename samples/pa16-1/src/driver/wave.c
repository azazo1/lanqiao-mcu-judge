#include "wave.h"

sbit Tx = P1^0;
sbit Rx = P1^1;

void Delay12us(void)	//@12.000MHz
{
	unsigned char data i;

	_nop_();
	_nop_();
	i = 33;
	while (--i);
}

void waveInit()
{
	unsigned char i;
	for(i = 0; i < 8; i++)
	{
		Tx = 1;
		Delay12us();
		Tx = 0;
		Delay12us();
	}
}

unsigned char waveGet()
{
	unsigned int time;
	CMOD = 0x00;
	CH = CL = 0;
	waveInit();
	CR = 1;
	while(!CF && Rx);
	CR = 0;
	if(CF)
	{
		CF = 0;
		return 0;
	}
	else
	{
		time = (CH << 8) | CL;
		return (time * 0.017);
	}
}