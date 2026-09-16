#include <led.h>

void LedDisp(unsigned char *ucLed)
{
	unsigned char i, temp = 0x00;
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

void Relay(bit enable)
{
	unsigned char temp1 = 0x00;
	static unsigned char temp1_old = 0xff;
	
	if(enable)
		temp1 |= 0x10;
	else
		temp1 &= ~0x10;
	
	if(temp1 != temp1_old)
	{
		P0 = temp1;
		P2 = P2 & 0x1f | 0xa0;
		P2 &= 0x1f;
		temp1_old = temp1;
	}
}