#include "pcf8591.h"
#include "iic.h"

u8 adc(u8 addr) {
  u8 t1, t2;
	do {
		I2CStart();
		I2CSendByte(0x90);
		I2CWaitAck();
		I2CSendByte(addr);
		I2CWaitAck();

		I2CStart();
		I2CSendByte(0x91);
		I2CWaitAck();
		t1 = I2CReceiveByte();
		I2CSendAck(1);
		I2CStop();
		
		I2CStart();
		I2CSendByte(0x90);
		I2CWaitAck();
		I2CSendByte(addr);
		I2CWaitAck();

		I2CStart();
		I2CSendByte(0x91);
		I2CWaitAck();
		t2 = I2CReceiveByte();
		I2CSendAck(1);
		I2CStop();
	} while (t1 != t2);
  return t1;
}

void dac(u8 lvl) {
  I2CStart();
  I2CSendByte(0x90);
  I2CWaitAck();
  I2CSendByte(0x41);
  I2CWaitAck();
  I2CSendByte(lvl);
  I2CWaitAck();
  I2CStop();
}