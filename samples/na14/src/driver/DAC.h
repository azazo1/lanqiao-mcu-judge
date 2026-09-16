#ifndef __DAC_H__
#define __DAC_H__

void I2CStart(void);
void I2CStop(void);
void I2CSendByte(unsigned char byt);
unsigned char I2CWaitAck(void);
void DAC(unsigned char Date);

#endif