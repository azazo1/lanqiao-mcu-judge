#include <STC15F2K60S2.H>
#include <intrins.h>

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

void Wave_Init()
{
	unsigned char i;
	EA = 0;
	for(i = 0; i < 8; i++)
	{
		Tx = 1;
		Delay12us();
		Tx = 0;
		Delay12us();
	}
	EA = 1;
}

unsigned char Wave_Cm(unsigned int v)
{
	unsigned int time;
	CMOD = 0x00;
	CL = CH = 0;
	Wave_Init();
	CR = 1;
	while(Rx == 1 && CF == 0);
	CR = 0;
	if(!CF)
	{
		time = CH << 8 | CL;
		return (float)(time / 20000.0 * v);
	}
	else
	{
    CF = 0;
    return 0;
  }
}

