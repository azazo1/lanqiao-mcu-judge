#include <key.h>

unsigned char KeyDisp()
{
	unsigned char temp = 0;
	AUXR &= ~(0x10);
	P44 = 1;
	P42 = 1;
	P35 = 0;
	P34 = 1;
	if(P33 == 0) temp = 12;
	if(P32 == 0) temp = 13;
	
	P44 = 1;
	P42 = 1;
	P35 = 1;
	P34 = 0;
	if(P33 == 0) temp = 16;
	if(P32 == 0) temp = 17;
	
	P3 = 0xff; 
	AUXR |= 0x10;
	return temp;
}