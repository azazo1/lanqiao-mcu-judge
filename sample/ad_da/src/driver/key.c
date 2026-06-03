#include "key.h"

u8 key_read() {
  u8 t;
  // BTN mode
  if (P33 == 0)
    t = 4;
  if (P32 == 0)
    t = 5;
  if (P31 == 0)
    t = 6;
  if (P30 == 0)
    t = 7;
  return t;
}