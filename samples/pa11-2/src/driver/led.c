#include "led.h"

idata unsigned char temp=0x00;
idata unsigned char temp_old=0xff;

void Led_Disp(unsigned char *ucLed)
{
	temp=0x00;
	temp=(ucLed[0]<<0)|(ucLed[1]<<1)|(ucLed[2]<<2)|(ucLed[3]<<3)|
			 (ucLed[4]<<4)|(ucLed[5]<<5)|(ucLed[6]<<6)|(ucLed[7]<<7);
	if(temp!=temp_old)
	{
		P0 = ~temp;
		P2 = (P2&0x1f) | 0x80;
		P2 &= 0x1f;
		temp_old=temp;
	}
}

void Led_Off()
{
	P0 = 0xff;
	P2 = (P2&0x1f) | 0x80;
	P2 &= 0x1f;
	temp_old=0x00;
}