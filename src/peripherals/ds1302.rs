use crate::peripherals::Rtc;

#[derive(Debug, Default)]
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
    pub(crate) last_write_reg: u8,
    pub(crate) last_write_value: u8,
    pub(crate) last_clock_write_reg: u8,
    pub(crate) last_clock_write_value: u8,
    pub(crate) last_read_reg: u8,
    pub(crate) last_read_value: u8,
}

impl Ds1302 {
    pub(crate) fn sample(&mut self, ce: bool, clk: bool, io: bool, rtc: &mut Rtc) {
        if !ce {
            self.ce_prev = ce;
            self.clk_prev = clk;
            self.bit_count = 0;
            self.shift_in = 0;
            self.shift_out = 0;
            self.read_byte = 0;
            self.reading = false;
            self.data_phase = false;
            self.burst_index = 0;
            self.io_level = true;
            return;
        }

        if !self.ce_prev && ce {
            self.bit_count = 0;
            self.shift_in = 0;
            self.shift_out = 0;
            self.read_byte = 0;
            self.reading = false;
            self.data_phase = false;
            self.burst_index = 0;
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
                        self.read_byte = self.read_register(rtc);
                        self.last_read_reg = self.effective_reg();
                        self.last_read_value = self.read_byte;
                        self.shift_out = self.read_byte;
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
                    self.write_register(value, rtc);
                    if self.is_clock_burst() {
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
                if self.is_clock_burst() {
                    self.burst_index = self.burst_index.saturating_add(1);
                    self.read_byte = self.read_register(rtc);
                    self.last_read_reg = self.effective_reg();
                    self.last_read_value = self.read_byte;
                }
                self.shift_out = self.read_byte;
            }
        }

        self.ce_prev = ce;
        self.clk_prev = clk;
    }

    fn is_clock_burst(&self) -> bool {
        self.current_reg & 0xFE == 0xBE
    }

    fn effective_reg(&self) -> u8 {
        if self.is_clock_burst() {
            match self.burst_index {
                0 => 0x80,
                1 => 0x82,
                2 => 0x84,
                3 => 0x86,
                4 => 0x88,
                5 => 0x8A,
                6 => 0x8C,
                7 => 0x8E,
                _ => 0x8E,
            }
        } else {
            self.current_reg & 0xFE
        }
    }

    fn read_register(&self, rtc: &Rtc) -> u8 {
        match self.effective_reg() {
            0x80 => bcd(rtc.second),
            0x82 => bcd(rtc.minute),
            0x84 => encode_hour(rtc.hour),
            0x86 => 1,
            0x88 => 1,
            0x8A => 1,
            0x8C => 0,
            0x8E => u8::from(self.write_protect) << 7,
            0x90 => 0,
            _ => 0,
        }
    }

    fn write_register(&mut self, value: u8, rtc: &mut Rtc) {
        let reg = self.effective_reg();
        if self.write_protect && reg != 0x8E {
            return;
        }
        match reg {
            0x80 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.second = decode_bcd(value & 0x7F).min(59);
                rtc.sub_us = 0;
            }
            0x82 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.minute = decode_bcd(value & 0x7F).min(59);
            }
            0x84 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.hour = decode_hour(value);
            }
            0x8E => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.write_protect = value & 0x80 != 0;
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

fn encode_hour(hour: u8) -> u8 {
    bcd(hour % 24)
}

fn decode_hour(value: u8) -> u8 {
    if value & 0x80 == 0 {
        return decode_bcd(value & 0x3F).min(23);
    }
    let mut hour = decode_bcd(value & 0x1F).clamp(1, 12);
    if value & 0x20 != 0 {
        hour = (hour % 12) + 12;
    } else if hour == 12 {
        hour = 0;
    }
    hour
}
