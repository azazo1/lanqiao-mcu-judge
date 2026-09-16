#include <STC15F2K60S2.H>


static unsigned char temp = 0x00, temp_old = 0xff;
	
void LED_Disp(unsigned char *ucLed)
{
	unsigned char i;
	
	temp = 0x00;
	
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

static unsigned char temp1 = 0x00, temp1_old = 0xff;

void Relay(unsigned char enable)
{
	temp1 = 0x00;
	
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
