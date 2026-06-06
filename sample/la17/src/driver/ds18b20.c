#include "ds18b20.h"

/*	# 	单总线代码片段说明
	1. 	本文件夹中提供的驱动代码供参赛选手完成程序设计参考。
	2. 	参赛选手可以自行编写相关代码或以该代码为基础，根据所选单片机类型、运行速度和试题
		中对单片机时钟频率的要求，进行代码调试和修改。
*/
sbit DQ = P1^4;
//
void Delay_OneWire(unsigned int t)  
{
	unsigned char i;
	while(t--){
		for(i=0;i<12;i++);
	}
}

//
void Write_DS18B20(unsigned char dat)
{
	unsigned char i;
	for(i=0;i<8;i++)
	{
		DQ = 0;
		DQ = dat&0x01;
		Delay_OneWire(5);
		DQ = 1;
		dat >>= 1;
	}
	Delay_OneWire(5);
}

//
unsigned char Read_DS18B20(void)
{
	unsigned char i;
	unsigned char dat;
  
	for(i=0;i<8;i++)
	{
		DQ = 0;
		dat >>= 1;
		DQ = 1;
		if(DQ)
		{
			dat |= 0x80;
		}	    
		Delay_OneWire(5);
	}
	return dat;
}

//
bit init_ds18b20(void)
{
  	bit initflag = 0;
  	
  	DQ = 1;
  	Delay_OneWire(12);
  	DQ = 0;
  	Delay_OneWire(80);
  	DQ = 1;
  	Delay_OneWire(10); 
    initflag = DQ;     
  	Delay_OneWire(5);
  
  	return initflag;
}


float tempe_get() {
	u8 l1, h1, l2, h2;
	do {
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0x44);
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);
		
		l1 = Read_DS18B20();
		h1 = Read_DS18B20();
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0x44);
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);
		
		l2 = Read_DS18B20();
		h2 = Read_DS18B20();
	} while (l1 != l2 || h1 != h2);
	return (h1 << 8 | l1) * 0.0625;
}

u8 tempe_get_res() {
	u8 t1, t2;
	do {
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);
		
		Read_DS18B20();
		Read_DS18B20();
		Read_DS18B20();
		Read_DS18B20();
		t1 = Read_DS18B20();
		
		init_ds18b20();
		Write_DS18B20(0xCC);
		Write_DS18B20(0xBE);

		Read_DS18B20();
		Read_DS18B20();
		Read_DS18B20();
		Read_DS18B20();
		t2 = Read_DS18B20();
	} while (t1 != t2);
	return (t1 >> 5) & 0x03;
}

void tempe_set_res(u8 level) {
	init_ds18b20();
	Write_DS18B20(0xCC);
	Write_DS18B20(0x4E);
	Write_DS18B20(0);
	Write_DS18B20(0);
	Write_DS18B20((level << 5) | 0x1f);
	
	init_ds18b20();
	Write_DS18B20(0xCC);
	Write_DS18B20(0x48);
}