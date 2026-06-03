#include "init.h"

void sys_init() {
  write_p0(0xff, 0x80);
  write_p0(0x00, 0xa0);
}