#include "key.h"

uint key_read() {
	uint t = 0;
	
	P44 = 0; P42 = 1; P35 = 1;
	if (P33 == 0) t |= 1 << 0;
	if (P32 == 0) t |= 1 << 1;
	
	P44 = 1; P42 = 0; P35 = 1;
	if (P33 == 0) t |= 1 << 4;
	if (P32 == 0) t |= 1 << 5;
	
	P44 = 1; P42 = 1; P35 = 0;
	if (P33 == 0) t |= 1 << 8;
	if (P32 == 0) t |= 1 << 9;
	
	return t;
}