#include <STC15F2K60S2.H>
#include <intrins.h>

void DaWrite(unsigned char dat);
void EepromRead(unsigned char *string,unsigned char add,unsigned char num);
void EepromWrite(unsigned char *string,unsigned char add,unsigned char num);