#include <STC15F2K60S2.H>

unsigned char Key_Disp(void)
{
	unsigned char temp = 0;
	
	P44 = 0;P42 = 1;P35 = 1;P34 = 1;
	if(P32 == 0) temp = 5;
	if(P33 == 0) temp = 4;
	
	P44 = 1;P42 = 0;P35 = 1;P34 = 1;
	if(P32 == 0) temp = 9;
	if(P33 == 0) temp = 8;
	if((P33 == 0) && (P32 == 0))	temp = 89;
	
	return temp;
}








