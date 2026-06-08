#include "init.h"

void sys_init() {
	write0(0x80, 0xff);
	write0(0xa0, 0x00);
}