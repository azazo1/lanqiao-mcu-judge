#include <STC15F2K60S2.H>
#include <intrins.h>

unsigned char AdRead();
void DaWrite(unsigned char dat);
void EepromWrite(unsigned char *string,unsigned char add,unsigned char num);