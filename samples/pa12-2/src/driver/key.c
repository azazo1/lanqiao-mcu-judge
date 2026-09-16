#include <key.h>

unsigned char KeyDisp()
{
	//独立按键模式！4T可能不会扣分，人工评判会扣分（如果有人工评判的话）
	unsigned char temp = 0;
	
	if(P33 == 0) temp = 4;
	if(P32 == 0) temp = 5;
	if(P31 == 0) temp = 6;
	if(P30 == 0) temp = 7;
	
	return temp;
}