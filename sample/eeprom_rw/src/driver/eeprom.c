#include "eeprom.h"
#include "iic.h"
void Delay5ms(void)	//@12.000MHz
{
	unsigned char data i, j;

	i = 59;
	j = 90;
	do
	{
		while (--j);
	} while (--i);
}

void eeprom_read(u8 addr, u8 *buf, u8 len) {
	I2CStart();
	I2CSendByte(0xA0);
	I2CWaitAck();
	I2CSendByte(addr);
	I2CWaitAck();
	
	I2CStart();
	I2CSendByte(0xA1);
	I2CWaitAck();
	while (len--) {
		*buf++ = I2CReceiveByte();
		I2CSendAck(!len);
	}
	I2CStop();
}

void eeprom_write(u8 addr, u8 *buf, u8 len) {
	I2CStart();
	I2CSendByte(0xA0);
	I2CWaitAck();
	I2CSendByte(addr);
	I2CWaitAck();
	
	while (len--) {
		I2CSendByte(*buf++);
		I2CWaitAck();
		I2C_Delay(200);
	}
	I2CStop();
	Delay5ms();
}