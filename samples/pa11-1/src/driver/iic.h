#include <STC15F2K60S2.H>
unsigned char Ad_Read(unsigned char addr);
void EEPROM_Write(unsigned char *str,unsigned char addr,unsigned char num);
void EEPROM_Read(unsigned char *str,unsigned char addr,unsigned char num);