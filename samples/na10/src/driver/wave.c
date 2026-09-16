#include <wave.h>

sbit Tx = P1^0;
sbit Rx = P1^1;

void Delay12us(void)	//@12.000MHz
{
	unsigned char data i;

	_nop_();
	_nop_();
	i = 38;
	while (--i);
}

void WaveInit()
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

unsigned char Wave()
{
	unsigned int time;
	CMOD = 0x00;
	CH = CL = 0;
	WaveInit();
	CR = 1;
	while((CF == 0) && (Rx == 1));
	CR = 0;
	if(CF == 0)//Òç³ö
	{
		time = (CH << 8) | CL;
		return (time * 0.017);
	}
	else
	{
		CF = 0;
		return 0;
	}
}