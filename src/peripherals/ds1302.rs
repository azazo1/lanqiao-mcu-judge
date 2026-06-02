use anyhow::{Result, bail};

use crate::timing::TICKS_PER_SECOND;

#[derive(Debug, Clone)]
pub(crate) struct Ds1302 {
    ce_prev: bool,
    clk_prev: bool,
    bit_count: u8,
    shift_in: u8,
    shift_out: u8,
    read_byte: u8,
    pub(crate) reading: bool,
    pub(crate) io_level: bool,
    pub(crate) current_reg: u8,
    data_phase: bool,
    burst_index: u8,
    write_protect: bool,
    trickle_charge: u8,
    ram: [u8; 31],
    hour_mode_12: bool,
    pub(crate) hour: u8,
    pub(crate) minute: u8,
    pub(crate) second: u8,
    pub(crate) day_of_week: u8,
    pub(crate) date: u8,
    pub(crate) month: u8,
    pub(crate) year: u8,
    pub(crate) halted: bool,
    pub(crate) sub_us: u64,
    pub(crate) last_write_reg: u8,
    pub(crate) last_write_value: u8,
    pub(crate) last_clock_write_reg: u8,
    pub(crate) last_clock_write_value: u8,
    pub(crate) last_read_reg: u8,
    pub(crate) last_read_value: u8,
}

impl Default for Ds1302 {
    fn default() -> Self {
        Self {
            ce_prev: false,
            clk_prev: false,
            bit_count: 0,
            shift_in: 0,
            shift_out: 0,
            read_byte: 0,
            reading: false,
            io_level: true,
            current_reg: 0,
            data_phase: false,
            burst_index: 0,
            write_protect: false,
            trickle_charge: 0,
            ram: [0; 31],
            hour_mode_12: false,
            hour: 0,
            minute: 0,
            second: 0,
            day_of_week: 1,
            date: 1,
            month: 1,
            year: 0,
            halted: false,
            sub_us: 0,
            last_write_reg: 0,
            last_write_value: 0,
            last_clock_write_reg: 0,
            last_clock_write_value: 0,
            last_read_reg: 0,
            last_read_value: 0,
        }
    }
}

impl Ds1302 {
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
        if self.halted {
            return;
        }

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
        self.hour += 1;
        if self.hour < 24 {
            return;
        }
        self.hour = 0;
        self.increment_date();
    }

    pub(crate) fn sample(&mut self, ce: bool, clk: bool, io: bool) {
        if !ce {
            self.reset_transfer_state();
            self.ce_prev = ce;
            self.clk_prev = clk;
            return;
        }

        if !self.ce_prev && ce {
            self.reset_transfer_state();
        }

        if !self.clk_prev && clk {
            if !self.data_phase {
                if io {
                    self.shift_in |= 1 << self.bit_count;
                }
                self.bit_count += 1;
                if self.bit_count == 8 {
                    self.current_reg = self.shift_in;
                    self.shift_in = 0;
                    self.bit_count = 0;
                    self.data_phase = true;
                    self.reading = self.current_reg & 0x01 != 0;
                    if self.reading {
                        self.load_read_byte();
                    }
                }
            } else if !self.reading {
                if io {
                    self.shift_in |= 1 << self.bit_count;
                }
                self.bit_count += 1;
                if self.bit_count == 8 {
                    let value = self.shift_in;
                    self.shift_in = 0;
                    self.bit_count = 0;
                    self.write_register(value);
                    if self.is_burst() {
                        self.burst_index = self.burst_index.saturating_add(1);
                    }
                }
            }
        }

        if self.clk_prev && !clk && self.data_phase && self.reading {
            self.io_level = self.shift_out & 1 != 0;
            self.shift_out >>= 1;
            self.bit_count += 1;
            if self.bit_count == 8 {
                self.bit_count = 0;
                if self.is_burst() {
                    self.burst_index = self.burst_index.saturating_add(1);
                    self.load_read_byte();
                }
                self.shift_out = self.read_byte;
            }
        }

        self.ce_prev = ce;
        self.clk_prev = clk;
    }

    fn increment_date(&mut self) {
        self.day_of_week = if self.day_of_week >= 7 {
            1
        } else {
            self.day_of_week + 1
        };

        self.date += 1;
        if self.date <= self.days_in_month() {
            return;
        }

        self.date = 1;
        self.month += 1;
        if self.month <= 12 {
            return;
        }

        self.month = 1;
        self.year = (self.year + 1) % 100;
    }

    fn days_in_month(&self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if self.year.is_multiple_of(4) => 29,
            2 => 28,
            _ => 31,
        }
    }

    fn reset_transfer_state(&mut self) {
        self.bit_count = 0;
        self.shift_in = 0;
        self.shift_out = 0;
        self.read_byte = 0;
        self.reading = false;
        self.data_phase = false;
        self.burst_index = 0;
        self.io_level = true;
    }

    fn load_read_byte(&mut self) {
        self.read_byte = self.read_register();
        self.last_read_reg = self.effective_reg();
        self.last_read_value = self.read_byte;
        self.shift_out = self.read_byte;
    }

    fn is_clock_burst(&self) -> bool {
        self.current_reg & 0xFE == 0xBE
    }

    fn is_ram_burst(&self) -> bool {
        self.current_reg & 0xFE == 0xFE
    }

    fn is_burst(&self) -> bool {
        self.is_clock_burst() || self.is_ram_burst()
    }

    fn effective_reg(&self) -> u8 {
        if self.is_clock_burst() {
            return match self.burst_index {
                0 => 0x80,
                1 => 0x82,
                2 => 0x84,
                3 => 0x86,
                4 => 0x88,
                5 => 0x8A,
                6 => 0x8C,
                7 => 0x8E,
                _ => 0x8E,
            };
        }

        if self.is_ram_burst() {
            return 0xC0 + self.burst_index.saturating_mul(2);
        }

        self.current_reg & 0xFE
    }

    fn read_register(&self) -> u8 {
        match self.effective_reg() {
            0x80 => bcd(self.second) | (u8::from(self.halted) << 7),
            0x82 => bcd(self.minute),
            0x84 => encode_hour(self.hour, self.hour_mode_12),
            0x86 => bcd(self.date),
            0x88 => bcd(self.month),
            0x8A => bcd(self.day_of_week),
            0x8C => bcd(self.year),
            0x8E => u8::from(self.write_protect) << 7,
            0x90 => self.trickle_charge,
            reg @ 0xC0..=0xFC if reg.is_multiple_of(2) => self.ram[((reg - 0xC0) / 2) as usize],
            _ => 0,
        }
    }

    fn write_register(&mut self, value: u8) {
        let reg = self.effective_reg();
        if self.write_protect && reg != 0x8E {
            return;
        }

        self.last_write_reg = reg;
        self.last_write_value = value;
        if reg < 0x90 {
            self.last_clock_write_reg = reg;
            self.last_clock_write_value = value;
        }

        match reg {
            0x80 => {
                self.halted = value & 0x80 != 0;
                self.second = decode_bcd(value & 0x7F).min(59);
                self.sub_us = 0;
            }
            0x82 => {
                self.minute = decode_bcd(value & 0x7F).min(59);
            }
            0x84 => {
                let (hour, hour_mode_12) = decode_hour(value);
                self.hour = hour;
                self.hour_mode_12 = hour_mode_12;
            }
            0x86 => {
                self.date = decode_bcd(value & 0x3F).clamp(1, 31);
            }
            0x88 => {
                self.month = decode_bcd(value & 0x1F).clamp(1, 12);
            }
            0x8A => {
                self.day_of_week = decode_bcd(value & 0x07).clamp(1, 7);
            }
            0x8C => {
                self.year = decode_bcd(value);
            }
            0x8E => {
                self.write_protect = value & 0x80 != 0;
            }
            0x90 => {
                self.trickle_charge = value;
            }
            reg @ 0xC0..=0xFC if reg.is_multiple_of(2) => {
                self.ram[((reg - 0xC0) / 2) as usize] = value;
            }
            _ => {}
        }
    }
}

fn bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

fn decode_bcd(value: u8) -> u8 {
    ((value >> 4) * 10) + (value & 0x0F)
}

fn encode_hour(hour: u8, hour_mode_12: bool) -> u8 {
    let hour = hour % 24;
    if !hour_mode_12 {
        return bcd(hour);
    }

    let pm = hour >= 12;
    let hour12 = match hour % 12 {
        0 => 12,
        value => value,
    };

    0x80 | (u8::from(pm) << 5) | bcd(hour12)
}

fn decode_hour(value: u8) -> (u8, bool) {
    if value & 0x80 == 0 {
        return (decode_bcd(value & 0x3F).min(23), false);
    }
    let mut hour = decode_bcd(value & 0x1F).clamp(1, 12);
    if value & 0x20 != 0 {
        hour = (hour % 12) + 12;
    } else if hour == 12 {
        hour = 0;
    }
    (hour, true)
}

#[cfg(test)]
mod tests {
    use super::Ds1302;
    use crate::timing::TICKS_PER_SECOND;

    #[test]
    fn reads_and_writes_calendar_registers() {
        let mut ds1302 = Ds1302 {
            current_reg: 0x86,
            ..Ds1302::default()
        };

        ds1302.write_register(0x28);
        ds1302.current_reg = 0x88;
        ds1302.write_register(0x12);
        ds1302.current_reg = 0x8A;
        ds1302.write_register(0x06);
        ds1302.current_reg = 0x8C;
        ds1302.write_register(0x24);

        assert_eq!(ds1302.date, 28);
        assert_eq!(ds1302.month, 12);
        assert_eq!(ds1302.day_of_week, 6);
        assert_eq!(ds1302.year, 24);

        assert_eq!(ds1302.read_register(), 0x24);
        ds1302.current_reg = 0x86;
        assert_eq!(ds1302.read_register(), 0x28);
    }

    #[test]
    fn stores_ram_bytes() {
        let mut ds1302 = Ds1302 {
            current_reg: 0xC0,
            ..Ds1302::default()
        };

        ds1302.write_register(0x5A);
        assert_eq!(ds1302.read_register(), 0x5A);
    }

    #[test]
    fn hour_register_round_trips_12h_and_pm_bits() {
        let mut ds1302 = Ds1302 {
            current_reg: 0x84,
            ..Ds1302::default()
        };

        ds1302.write_register(0x92);
        assert_eq!(ds1302.hour, 0);
        assert!(ds1302.hour_mode_12);
        assert_eq!(ds1302.read_register(), 0x92);

        ds1302.write_register(0xA1);
        assert_eq!(ds1302.hour, 13);
        assert!(ds1302.hour_mode_12);
        assert_eq!(ds1302.read_register(), 0xA1);

        ds1302.write_register(0x23);
        assert_eq!(ds1302.hour, 23);
        assert!(!ds1302.hour_mode_12);
        assert_eq!(ds1302.read_register(), 0x23);
    }

    #[test]
    fn ch_bit_stops_clock_tick() {
        let mut ds1302 = Ds1302 {
            current_reg: 0x80,
            ..Ds1302::default()
        };

        ds1302.write_register(0x80);
        assert!(ds1302.halted);

        for _ in 0..TICKS_PER_SECOND {
            ds1302.tick();
        }

        assert_eq!(ds1302.second, 0);
        assert_eq!(ds1302.read_register(), 0x80);
    }

    #[test]
    fn clearing_ch_via_second_write_resumes_counting() {
        let mut ds1302 = Ds1302 {
            current_reg: 0x80,
            ..Ds1302::default()
        };

        ds1302.write_register(0x80);
        assert!(ds1302.halted);
        assert_eq!(ds1302.second, 0);

        ds1302.write_register(0x25);
        assert!(!ds1302.halted);
        assert_eq!(ds1302.second, 25);
        assert_eq!(ds1302.read_register(), 0x25);

        for _ in 0..TICKS_PER_SECOND {
            ds1302.tick();
        }

        assert_eq!(ds1302.second, 26);
        assert_eq!(ds1302.read_register(), 0x26);
    }

    #[test]
    fn wp_bit_blocks_clock_writes_until_cleared() {
        let mut ds1302 = Ds1302 {
            current_reg: 0x80,
            ..Ds1302::default()
        };

        ds1302.write_register(0x80);
        assert_eq!(ds1302.second, 0);
        assert!(ds1302.halted);

        ds1302.current_reg = 0x8E;
        ds1302.write_register(0x80);
        assert!(ds1302.write_protect);

        ds1302.current_reg = 0x80;
        ds1302.write_register(0x45);
        assert_eq!(ds1302.second, 0);
        assert!(ds1302.halted);

        ds1302.current_reg = 0x8E;
        ds1302.write_register(0x00);
        assert!(!ds1302.write_protect);

        ds1302.current_reg = 0x80;
        ds1302.write_register(0x45);
        assert_eq!(ds1302.second, 45);
        assert!(!ds1302.halted);
    }

    #[test]
    fn rolls_date_across_year_boundary() {
        let mut ds1302 = Ds1302 {
            hour: 23,
            minute: 59,
            second: 59,
            day_of_week: 7,
            date: 31,
            month: 12,
            year: 99,
            halted: false,
            sub_us: TICKS_PER_SECOND - 1,
            ..Ds1302::default()
        };

        ds1302.tick();

        assert_eq!(ds1302.hour, 0);
        assert_eq!(ds1302.minute, 0);
        assert_eq!(ds1302.second, 0);
        assert_eq!(ds1302.day_of_week, 1);
        assert_eq!(ds1302.date, 1);
        assert_eq!(ds1302.month, 1);
        assert_eq!(ds1302.year, 0);
    }
}
