#include "utils.h"

void write_p0(u8 dat, u8 addr) {
  u8 t;
  P0 = dat;
  t = P2 & 0x1f;
  P2 = t | addr;
  P2 = t;
}