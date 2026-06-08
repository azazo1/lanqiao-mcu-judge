#include "ds18b20.h"
#include "onewire.h"

float tempe_get() {
	u8 hi1, lo1, hi2, lo2;
	do {
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0x44);
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);
		
		lo1 = Read_DS18B20();
		hi1 = Read_DS18B20();

		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0x44);
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);
		
		lo2 = Read_DS18B20();
		hi2 = Read_DS18B20();
	} while (hi1 != hi2 || lo1 != lo2);
	return ((hi1 << 8) | lo1) * 0.0625;
}

void tempe_set_resolution(u8 level) {
	switch (level) {
		case 0: level = 0x1F; break;
		case 1: level = 0x3F; break;
		case 2: level = 0x5F; break;
		case 3:
		default:
			level = 0x7F;
	}
	init_ds18b20();
	Write_DS18B20(0xCC);
	Write_DS18B20(0x4E);
	Write_DS18B20(0);
	Write_DS18B20(0);
	Write_DS18B20(level);
	
	init_ds18b20();
	Write_DS18B20(0xCC);
	Write_DS18B20(0x48);
}