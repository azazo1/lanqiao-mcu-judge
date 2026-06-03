use std::collections::VecDeque;

use super::{AnalogInputs, At24c02, Pcf8591};

#[derive(Debug, Default)]
pub(crate) struct I2cBus {
    scl_prev: bool,
    sda_prev: bool,
    bit_count: u8,
    shift: u8,
    write_ack_pending: bool,
    write_ack_clock_high: bool,
    read_ack_pending: bool,
    read_ack_clock_high: bool,
    pub(crate) sda_drive_low: bool,
    pub(crate) scl_drive_low: bool,
    active: Option<I2cDevice>,
    mode: I2cMode,
    read_buffer: VecDeque<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum I2cMode {
    #[default]
    Idle,
    Address,
    Write,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum I2cDevice {
    Pcf8591,
    Eeprom,
}

impl I2cBus {
    pub(crate) fn sample(
        &mut self,
        scl_high: bool,
        sda_high: bool,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
        if self.sda_prev && !sda_high && scl_high {
            self.mode = I2cMode::Address;
            self.bit_count = 0;
            self.shift = 0;
            self.write_ack_pending = false;
            self.write_ack_clock_high = false;
            self.read_ack_pending = false;
            self.read_ack_clock_high = false;
            self.read_buffer.clear();
            self.sda_drive_low = false;
        }
        if !self.sda_prev && sda_high && scl_high {
            self.mode = I2cMode::Idle;
            self.active = None;
            self.write_ack_pending = false;
            self.write_ack_clock_high = false;
            self.read_ack_pending = false;
            self.read_ack_clock_high = false;
            self.sda_drive_low = false;
        }

        if !self.scl_prev && scl_high {
            match self.mode {
                I2cMode::Address | I2cMode::Write => {
                    if self.write_ack_pending {
                        self.write_ack_clock_high = true;
                    } else {
                        self.shift = (self.shift << 1) | u8::from(sda_high);
                        self.bit_count += 1;
                        if self.bit_count == 8 {
                            self.handle_received_byte(analog, pcf8591, at24c02);
                            self.bit_count = 0;
                            self.shift = 0;
                            self.write_ack_pending = true;
                            self.write_ack_clock_high = false;
                            self.sda_drive_low = true;
                        }
                    }
                }
                I2cMode::Read => {
                    if self.read_ack_pending {
                        self.read_ack_clock_high = true;
                    } else {
                        if self.bit_count == 0
                            && let Some(byte) = self.read_buffer.front().copied()
                        {
                            self.shift = byte;
                        }
                        self.bit_count += 1;
                        let bit = self.shift & 0x80 != 0;
                        self.sda_drive_low = !bit;
                        self.shift <<= 1;
                        if self.bit_count == 8 {
                            self.bit_count = 0;
                            self.read_ack_pending = true;
                            self.read_ack_clock_high = false;
                        }
                    }
                }
                I2cMode::Idle => {}
            }
        }

        if self.scl_prev && !scl_high {
            if self.write_ack_pending && self.write_ack_clock_high {
                self.write_ack_pending = false;
                self.write_ack_clock_high = false;
                self.sda_drive_low = false;
            }
            if self.read_ack_pending {
                if self.read_ack_clock_high {
                    self.read_ack_pending = false;
                    self.read_ack_clock_high = false;
                    self.read_buffer.pop_front();
                    if self.read_buffer.is_empty() {
                        self.refill_read_buffer(analog, pcf8591, at24c02);
                    }
                } else {
                    self.sda_drive_low = false;
                }
            }
        }

        self.scl_prev = scl_high;
        self.sda_prev = sda_high;
    }

    fn handle_received_byte(
        &mut self,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
        match self.mode {
            I2cMode::Address => {
                let address = self.shift >> 1;
                let read = self.shift & 1 != 0;
                self.active = match address {
                    0x48 => Some(I2cDevice::Pcf8591),
                    0x50 => Some(I2cDevice::Eeprom),
                    _ => None,
                };

                match (self.active, read) {
                    (Some(I2cDevice::Pcf8591), true) => {
                        self.mode = I2cMode::Read;
                        self.read_buffer.push_back(pcf8591.read_byte(analog));
                    }
                    (Some(I2cDevice::Pcf8591), false) => {
                        pcf8591.begin_write();
                        self.mode = I2cMode::Write;
                    }
                    (Some(I2cDevice::Eeprom), true) => {
                        self.mode = I2cMode::Read;
                        self.read_buffer.push_back(at24c02.read_byte());
                    }
                    (Some(I2cDevice::Eeprom), false) => {
                        at24c02.begin_write();
                        self.mode = I2cMode::Write;
                    }
                    (None, _) => {
                        self.mode = I2cMode::Idle;
                    }
                }
            }
            I2cMode::Write => match self.active {
                Some(I2cDevice::Pcf8591) => {
                    pcf8591.write_byte(self.shift);
                }
                Some(I2cDevice::Eeprom) => {
                    at24c02.write_byte(self.shift);
                }
                None => {}
            },
            I2cMode::Idle | I2cMode::Read => {}
        }
    }

    fn refill_read_buffer(
        &mut self,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
        match self.active {
            Some(I2cDevice::Pcf8591) => self.read_buffer.push_back(pcf8591.read_byte(analog)),
            Some(I2cDevice::Eeprom) => self.read_buffer.push_back(at24c02.read_byte()),
            None => {}
        }
    }
}
