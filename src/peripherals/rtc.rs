use anyhow::{Result, bail};

use crate::timing::TICKS_PER_SECOND;

#[derive(Debug, Default)]
pub(crate) struct Rtc {
    pub(crate) hour: u8,
    pub(crate) minute: u8,
    pub(crate) second: u8,
    pub(crate) sub_us: u64,
}

impl Rtc {
    pub(crate) fn set_hms(&mut self, hour: u8, minute: u8, second: u8) -> Result<()> {
        if hour > 23 || minute > 59 || second > 59 {
            bail!("RTC 时间越界");
        }
        self.hour = hour;
        self.minute = minute;
        self.second = second;
        self.sub_us = 0;
        Ok(())
    }

    pub(crate) fn tick(&mut self) {
        self.sub_us += 1;
        if self.sub_us < TICKS_PER_SECOND {
            return;
        }
        self.sub_us = 0;
        self.second += 1;
        if self.second < 60 {
            return;
        }
        self.second = 0;
        self.minute += 1;
        if self.minute < 60 {
            return;
        }
        self.minute = 0;
        self.hour = (self.hour + 1) % 24;
    }
}
