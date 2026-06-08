#include "utils.h"
#include "iic.h"

void write0(u8 pos, u8 val) {
	P0 = val;
	P2 = P2 & 0x1f | pos;
	P2 &= 0x1f;
}