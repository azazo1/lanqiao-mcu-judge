#include "utils.h"

#define S2RI  0x01
#define S2TI  0x02
#define S2RB8 0x08
#define S2TB8 0x04

void Uart1_Init();
void Uart2_Init();
void Uart2_Init_115200_9Bit();
void Uart2_Send9Bit(u8 dat, bit b9);
