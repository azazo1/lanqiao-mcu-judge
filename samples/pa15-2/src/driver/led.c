#include "led.h"
 idata  unsigned char temp_1 = 0x00;
 idata  unsigned char temp_old_1 = 0xff;

void Led_Disp(unsigned char *ucLed)
{

   temp_1=0x00;
   temp_1 = (ucLed[0] << 0) | (ucLed[1] << 1) | (ucLed[2] << 2) | (ucLed[3] << 3) |
         (ucLed[4] << 4) | (ucLed[5] << 5) | (ucLed[6] << 6) | (ucLed[7] << 7);
  if (temp_1 != temp_old_1)
  {
    P0 = ~temp_1;
    P2 = (P2 & 0x1f) | 0x80;
    P2 &= 0x1f;
    temp_old_1 = temp_1;
  }
}

void Led_Off()
{
	
	  P0 = 0xff;
    P2 = (P2 & 0x1f) | 0x80;
    P2 &= 0x1f;
	  temp_old_1=0x00;
}

idata unsigned char temp_0 = 0x00;
idata unsigned char temp_old_0 = 0xff;
void Relay(bit enable)
{
  if (enable)
    temp_0 |= 0x10;
  else
    temp_0 &= ~(0x10);
  if (temp_0 != temp_old_0)
  {
    P0 = temp_0;
    P2 = P2 & 0x1f | 0xa0;
    P2 &= 0x1f;
    temp_old_0 = temp_0;
  }
}
