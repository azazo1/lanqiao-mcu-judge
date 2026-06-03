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
    enter_read_after_ack: bool,
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
            self.enter_read_after_ack = false;
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
            self.enter_read_after_ack = false;
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
                if self.enter_read_after_ack {
                    self.enter_read_after_ack = false;
                    self.mode = I2cMode::Read;
                }
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
                        self.enter_read_after_ack = true;
                        self.read_buffer.push_back(pcf8591.read_byte(analog));
                    }
                    (Some(I2cDevice::Pcf8591), false) => {
                        pcf8591.begin_write();
                        self.mode = I2cMode::Write;
                    }
                    (Some(I2cDevice::Eeprom), true) => {
                        self.enter_read_after_ack = true;
                        self.read_buffer.push_back(at24c02.read_byte());
                    }
                    (Some(I2cDevice::Eeprom), false) => {
                        at24c02.begin_write();
                        self.mode = I2cMode::Write;
                    }
                    (None, _) => {
                        self.enter_read_after_ack = false;
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

#[cfg(test)]
mod tests {
    use super::I2cBus;
    use crate::peripherals::{AnalogInputs, At24c02, Pcf8591};

    struct I2cHarness {
        bus: I2cBus,
        analog: AnalogInputs,
        pcf8591: Pcf8591,
        at24c02: At24c02,
        scl: bool,
        sda: bool,
    }

    impl Default for I2cHarness {
        fn default() -> Self {
            let mut harness = Self {
                bus: I2cBus::default(),
                analog: AnalogInputs::default(),
                pcf8591: Pcf8591::default(),
                at24c02: At24c02::default(),
                scl: true,
                sda: true,
            };
            harness.tick();
            harness
        }
    }

    impl I2cHarness {
        fn tick(&mut self) {
            self.bus.sample(
                self.scl,
                self.sda,
                &self.analog,
                &mut self.pcf8591,
                &mut self.at24c02,
            );
        }

        fn set_scl(&mut self, level: bool) {
            self.scl = level;
            self.tick();
        }

        fn set_sda(&mut self, level: bool) {
            self.sda = level;
            self.tick();
        }

        fn line_sda_high(&self) -> bool {
            self.sda && !self.bus.sda_drive_low
        }

        fn start(&mut self) {
            self.set_sda(true);
            self.set_scl(true);
            self.set_sda(false);
            self.set_scl(false);
        }

        fn stop(&mut self) {
            self.set_sda(false);
            self.set_scl(true);
            self.set_sda(true);
        }

        fn send_byte(&mut self, byte: u8) {
            let mut value = byte;
            for _ in 0..8 {
                self.set_scl(false);
                self.set_sda(value & 0x80 != 0);
                self.set_scl(true);
                value <<= 1;
            }
            self.set_scl(false);
        }

        fn wait_ack(&mut self) -> bool {
            self.set_scl(true);
            let ack = self.line_sda_high();
            self.set_scl(false);
            ack
        }

        fn receive_byte(&mut self) -> u8 {
            let mut value = 0_u8;
            self.set_sda(true);
            for _ in 0..8 {
                self.set_scl(true);
                value = (value << 1) | u8::from(self.line_sda_high());
                self.set_scl(false);
            }
            value
        }

        fn send_ack(&mut self, ack: bool) {
            self.set_scl(false);
            self.set_sda(ack);
            self.set_scl(true);
            self.set_scl(false);
            self.set_sda(true);
        }

        fn eeprom_write_byte(&mut self, addr: u8, value: u8) {
            self.start();
            self.send_byte(0xA0);
            assert!(!self.wait_ack());
            self.send_byte(addr);
            assert!(!self.wait_ack());
            self.send_byte(value);
            assert!(!self.wait_ack());
            self.stop();
        }

        fn eeprom_read_byte(&mut self, addr: u8) -> u8 {
            self.start();
            self.send_byte(0xA0);
            assert!(!self.wait_ack());
            self.send_byte(addr);
            assert!(!self.wait_ack());
            self.start();
            self.send_byte(0xA1);
            assert!(!self.wait_ack());
            let value = self.receive_byte();
            self.send_ack(true);
            self.stop();
            value
        }
    }

    #[test]
    fn eeprom_random_read_returns_written_values() {
        let mut harness = I2cHarness::default();
        harness.eeprom_write_byte(0x00, 0x01);
        harness.eeprom_write_byte(0x01, 0x02);
        harness.eeprom_write_byte(0x02, 0x03);
        harness.eeprom_write_byte(0x20, 0x09);

        assert_eq!(harness.eeprom_read_byte(0x00), 0x01);
        assert_eq!(harness.eeprom_read_byte(0x01), 0x02);
        assert_eq!(harness.eeprom_read_byte(0x02), 0x03);
        assert_eq!(harness.eeprom_read_byte(0x20), 0x09);
        assert_eq!(harness.eeprom_read_byte(0x00), 0x01);
    }
}
