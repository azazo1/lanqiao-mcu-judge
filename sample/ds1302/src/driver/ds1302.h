#include "utils.h"

void rtc_set_time(u8 *buf);
void rtc_get_time(u8 *buf);
void rtc_set_date(u8 *buf);
void rtc_get_date(u8 *buf);
void rtc_set_12h(bit to12);
u8 rtc_get_12h();