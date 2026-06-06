#include "ds1302.h"
sbit SDA = P2^3;
sbit RST = P1^3;
sbit SCK = P1^7;
/*	# 	DS1302代码片段说明
	1. 	本文件夹中提供的驱动代码供参赛选手完成程序设计参考。
	2. 	参赛选手可以自行编写相关代码或以该代码为基础，根据所选单片机类型、运行速度和试题
		中对单片机时钟频率的要求，进行代码调试和修改。
*/								

//
void Write_Ds1302(unsigned  char temp) 
{
	unsigned char i;
	for (i=0;i<8;i++)     	
	{ 
		SCK = 0;
		SDA = temp&0x01;
		temp>>=1; 
		SCK=1;
	}
}   

//
void Write_Ds1302_Byte( unsigned char address,unsigned char dat )     
{
 	RST=0;	_nop_();
 	SCK=0;	_nop_();
 	RST=1; 	_nop_();  
 	Write_Ds1302(address);	
 	Write_Ds1302(dat);		
 	RST=0; 
}

//
unsigned char Read_Ds1302_Byte ( unsigned char address )
{
 	unsigned char i,temp=0x00;
 	RST=0;	_nop_();
 	SCK=0;	_nop_();
 	RST=1;	_nop_();
 	Write_Ds1302(address);
 	for (i=0;i<8;i++) 	
 	{		
		SCK=0;
		temp>>=1;	
 		if(SDA)
 		temp|=0x80;	
 		SCK=1;
	} 
 	RST=0;	_nop_();
 	SCK=0;	_nop_();
	SCK=1;	_nop_();
	SDA=0;	_nop_();
	SDA=1;	_nop_();
	return (temp);			
}

void rtc_set_time(u8 *buf) {
	Write_Ds1302_Byte(0x8e, 0x00);
	Write_Ds1302_Byte(0x80, 0x80);
	Write_Ds1302_Byte(0x84, buf[0] / 10 * 16 + (buf[0] % 10));
	Write_Ds1302_Byte(0x82, buf[1] / 10 * 16 + (buf[1] % 10));
	Write_Ds1302_Byte(0x80, buf[2] / 10 * 16 + (buf[2] % 10));
	Write_Ds1302_Byte(0x8e, 0x80);
}

void rtc_get_time(u8 *buf) {
	u8 h1, m1, s1, h2, m2, s2;
	
	do {
		h1 = Read_Ds1302_Byte(0x85);
		m1 = Read_Ds1302_Byte(0x83);
		s1 = Read_Ds1302_Byte(0x81);
		
		h2 = Read_Ds1302_Byte(0x85);
		m2 = Read_Ds1302_Byte(0x83);
		s2 = Read_Ds1302_Byte(0x81);
	} while (h1 != h2 || m1 != m2 || s1 != s2);

	if (h1 & 0x80) { // 12
		h1 &= 0x1f;
	}
	s1 &= 0x7f;

	buf[0] = h1 / 16 * 10 + (h1 % 16);
	buf[1] = m1 / 16 * 10 + (m1 % 16);
	buf[2] = s1 / 16 * 10 + (s1 % 16);
}

void rtc_set_date(u8 *buf) {
	Write_Ds1302_Byte(0x8e, 0x00);
	Write_Ds1302_Byte(0x8C, buf[0] / 10 * 16 + (buf[0] % 10)); // year
	Write_Ds1302_Byte(0x88, buf[1] / 10 * 16 + (buf[1] % 10)); // month
	Write_Ds1302_Byte(0x86, buf[2] / 10 * 16 + (buf[2] % 10)); // date
	Write_Ds1302_Byte(0x8A, buf[3] % 10); // day
	Write_Ds1302_Byte(0x8e, 0x80);
}

void rtc_get_date(u8 *buf) {
	u8 y, m, d;
	y = Read_Ds1302_Byte(0x8D);
	m = Read_Ds1302_Byte(0x89);
	d = Read_Ds1302_Byte(0x87);
	buf[3] = Read_Ds1302_Byte(0x8B);
	buf[0] = y / 16 * 10 + (y % 16);
	buf[1] = m / 16 * 10 + (m % 16);
	buf[2] = d / 16 * 10 + (d % 16);
}

void rtc_set_12h(bit t12) { // to 12 mode
	u8 ch = Read_Ds1302_Byte(0x85); // current hour (bcd, raw byte)
	u8 th, pm; // target (dec -> bcd), now is afternoon
	u8 c12 = (ch & 0x80) ? 1 : 0; // current is 12 mode

	if (t12 == c12) return;

	if (c12) {
		pm = (ch & 0x20) ? 1 : 0;
		th = ((ch & 0x10) >> 4) * 10 + (ch & 0x0f);
		if (pm) {
			if (th != 12) th += 12;
		} else if (th == 12) {
			th = 0;
		}
	} else {
		th = ((ch & 0x30) >> 4) * 10 + (ch & 0x0f);
		pm = th >= 12;
		if (th == 0) {
			th += 12;
		} else if (th <= 12) {
		} else if (th <= 23) {
			th -= 12;
		}
	}

	th = (t12 ? 0x80 : 0) | ((t12 & pm) ? 0x20 : 0x00) | ((th / 10) << 4) | (th % 10);
	Write_Ds1302_Byte(0x8E, 0x00);
	Write_Ds1302_Byte(0x84, th);
	Write_Ds1302_Byte(0x8E, 0x80);
}

u8 rtc_get_12h() {
	return 0x80 & Read_Ds1302_Byte(0x85) ? 0x01 : 0x00;
}
