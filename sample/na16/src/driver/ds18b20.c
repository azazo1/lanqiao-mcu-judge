#include "ds18b20.h"
#include "onewire.h"

float read_t() {
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
	} while (lo1 != lo2 && hi1 != hi2);
	return ((int)hi1 << 8 | (int)lo1) * 0.0625;
}