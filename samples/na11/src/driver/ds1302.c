#include <STC15F2K60S2.H>
#include <intrins.h>

sbit SCK = P1^7;
sbit RST = P1^3;
sbit SDA = P2^3;

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

void Write_Ds1302_Byte( unsigned char address,unsigned char dat )     
{
 	RST=0;	_nop_();
 	SCK=0;	_nop_();
 	RST=1; 	_nop_();  
 	Write_Ds1302(address);	
 	Write_Ds1302(dat);		
 	RST=0; 
}

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

code unsigned char DS1302_Arr[4] = {0x84,0x82,0x80,0x8E};

void SetRtc(unsigned char *Rtc)
{
	unsigned char i;
	Write_Ds1302_Byte(DS1302_Arr[3],0x00);
	
	for(i = 0; i < 3; i++)
		Write_Ds1302_Byte(DS1302_Arr[i],Rtc[i]);
	
	Write_Ds1302_Byte(DS1302_Arr[3],0x80);
}

void GetRtc(unsigned char *Rtc)
{
	unsigned char i;
	
	for(i = 0; i < 3; i++)
		Rtc[i] = Read_Ds1302_Byte(DS1302_Arr[i]+1);
}