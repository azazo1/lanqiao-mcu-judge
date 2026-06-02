#include "key.h"

uint key_read() {
	uint t = 0;
	if (P33 == 0) t |= 1 << 0;
	if (P32 == 0) t |= 1 << 1;
	if (P31 == 0) t |= 1 << 2;
	if (P30 == 0) t |= 1 << 3;

	return t;
}