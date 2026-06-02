use std::collections::VecDeque;

use super::AnalogInputs;

#[derive(Debug, Default)]
pub(crate) struct I2cBus {
    scl_prev: bool,
    sda_prev: bool,
    bit_count: u8,
    shift: u8,
    pub(crate) sda_drive_low: bool,
    pub(crate) scl_drive_low: bool,
    active: Option<I2cDevice>,
    mode: I2cMode,
    pcf8591_control: u8,
    eeprom_addr: u8,
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
    pub(crate) fn sample(&mut self, scl_high: bool, sda_high: bool, analog: &AnalogInputs) {
        if self.sda_prev && !sda_high && scl_high {
            self.mode = I2cMode::Address;
            self.bit_count = 0;
            self.shift = 0;
            self.read_buffer.clear();
            self.sda_drive_low = false;
        }
        if !self.sda_prev && sda_high && scl_high {
            self.mode = I2cMode::Idle;
            self.active = None;
            self.sda_drive_low = false;
        }

        if !self.scl_prev && scl_high {
            match self.mode {
                I2cMode::Address | I2cMode::Write => {
                    self.shift = (self.shift << 1) | u8::from(sda_high);
                    self.bit_count += 1;
                    if self.bit_count == 8 {
                        self.handle_received_byte(analog);
                        self.bit_count = 0;
                        self.shift = 0;
                        self.sda_drive_low = true;
                    }
                }
                I2cMode::Read => {
                    if self.bit_count == 0
                        && let Some(byte) = self.read_buffer.front().copied()
                    {
                        self.shift = byte;
                    }
                    self.bit_count += 1;
                    if self.bit_count > 8 {
                        self.bit_count = 0;
                        self.sda_drive_low = false;
                        self.read_buffer.pop_front();
                    } else {
                        let bit = self.shift & 0x80 != 0;
                        self.sda_drive_low = !bit;
                        self.shift <<= 1;
                    }
                }
                I2cMode::Idle => {}
            }
        }

        if self.scl_prev && !scl_high && self.sda_drive_low {
            self.sda_drive_low = false;
        }

        self.scl_prev = scl_high;
        self.sda_prev = sda_high;
    }

    fn handle_received_byte(&mut self, analog: &AnalogInputs) {
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
                        let value = analog.channel_value(self.pcf8591_control & 0x03);
                        self.read_buffer.push_back(value);
                    }
                    (Some(I2cDevice::Pcf8591), false) => {
                        self.mode = I2cMode::Write;
                    }
                    (Some(I2cDevice::Eeprom), true) => {
                        self.mode = I2cMode::Read;
                        self.read_buffer
                            .push_back(analog.eeprom[self.eeprom_addr as usize]);
                    }
                    (Some(I2cDevice::Eeprom), false) => {
                        self.mode = I2cMode::Write;
                    }
                    (None, _) => {
                        self.mode = I2cMode::Idle;
                    }
                }
            }
            I2cMode::Write => match self.active {
                Some(I2cDevice::Pcf8591) => {
                    self.pcf8591_control = self.shift;
                }
                Some(I2cDevice::Eeprom) => {
                    self.eeprom_addr = self.shift;
                }
                None => {}
            },
            I2cMode::Idle | I2cMode::Read => {}
        }
    }
}
