#include "led.h"

void ledDisp(unsigned char *ucLed)
{
	unsigned char temp = 0x00, i;
	static unsigned char temp_old = 0xff;
	
	for(i = 0; i < 8; i++)
		temp |= (ucLed[i] << i);
	
	if(temp != temp_old)
	{
		P0 = ~temp;
		P2 = P2 & 0x1f | 0x80;
		P2 &= 0x1f;
		temp_old = temp;
	}
}

void relayDisp(bit flag)
{
	unsigned char temp = 0x00;
	static unsigned char temp_old = 0xff;
	
	if(flag)
		temp |= 0x10;
	else
		temp &= ~0x10;
	
	if(temp != temp_old)
	{
		P0 = temp;
		P2 = P2 & 0x1f | 0xa0;
		P2 &= 0x1f;
		temp_old = temp;
	}
}