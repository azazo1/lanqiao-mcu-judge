use std::collections::VecDeque;

use tracing::trace;

use super::{AnalogInputs, At24c02, Pcf8591};

#[derive(Debug, Default)]
pub(crate) struct I2cBus {
    initialized: bool,
    scl_filter: SclFilter,
    master_sda_prev: bool,
    bit_count: u8,
    shift: u8,
    write_ack_pending: bool,
    write_ack_clock_high: bool,
    read_ack_pending: bool,
    read_ack_clock_high: bool,
    read_ack_master_high: bool,
    pub(crate) sda_drive_low: bool,
    pub(crate) scl_drive_low: bool,
    active: Option<I2cDevice>,
    mode: I2cMode,
    enter_read_after_ack: bool,
    read_buffer: VecDeque<u8>,
}

#[derive(Debug, Default)]
struct SclFilter {
    stable_high: bool,
    stable_since_ns: u64,
    raw_high: bool,
    raw_since_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SclTransition {
    Rising,
    Falling,
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
    const MIN_SCL_LOW_NS: u64 = 1_500;
    const MIN_SCL_HIGH_NS: u64 = 1_500;
    const MIN_START_STOP_SCL_HIGH_NS: u64 = 1_500;

    pub(crate) fn line_levels(&self, master_scl_high: bool, master_sda_high: bool) -> (bool, bool) {
        (
            master_scl_high && !self.scl_drive_low,
            master_sda_high && !self.sda_drive_low,
        )
    }

    pub(crate) fn sample(
        &mut self,
        time_ns: u64,
        master_scl_high: bool,
        master_sda_high: bool,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
        let (_, sda_high) = self.line_levels(master_scl_high, master_sda_high);

        if !self.initialized {
            self.initialized = true;
            self.master_sda_prev = master_sda_high;
            self.scl_filter.init(time_ns, master_scl_high);
            return;
        }

        let raw_scl_prev = self.scl_filter.raw_high;
        if master_scl_high != raw_scl_prev {
            trace!(time_ns, raw_scl = master_scl_high, "i2c raw scl");
        }
        if self.master_sda_prev != master_sda_high {
            trace!(time_ns, raw_sda = master_sda_high, "i2c raw sda");
        }

        if !raw_scl_prev && master_scl_high {
            if self.write_ack_pending {
                self.write_ack_clock_high = true;
                self.sda_drive_low = true;
            }
            if self.read_ack_pending {
                self.read_ack_clock_high = true;
                self.read_ack_master_high = master_sda_high;
            }
        }
        if raw_scl_prev && !master_scl_high && self.write_ack_pending && self.write_ack_clock_high {
            self.sda_drive_low = false;
        }

        let rise_filter_ns = if self.write_ack_pending || self.read_ack_pending {
            0
        } else {
            Self::MIN_SCL_HIGH_NS
        };
        if let Some(transition) =
            self.scl_filter
                .observe(time_ns, master_scl_high, rise_filter_ns, Self::MIN_SCL_LOW_NS)
        {
            match transition {
                SclTransition::Rising => {
                    self.handle_scl_rising(sda_high, master_sda_high, analog, pcf8591, at24c02);
                }
                SclTransition::Falling => self.handle_scl_falling(analog, pcf8591, at24c02),
            }
        }

        if self.master_sda_prev != master_sda_high
            && self.scl_filter.high_stable_for_ns(time_ns) >= Self::MIN_START_STOP_SCL_HIGH_NS
        {
            if self.master_sda_prev && !master_sda_high {
                trace!(time_ns, "i2c start");
                self.begin_start_condition();
            } else if !self.master_sda_prev && master_sda_high {
                trace!(time_ns, "i2c stop");
                self.finish_stop_condition();
            }
        }
        self.master_sda_prev = master_sda_high;
    }

    fn handle_scl_rising(
        &mut self,
        sda_high: bool,
        master_sda_high: bool,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
        match self.mode {
            I2cMode::Address | I2cMode::Write => {
                if self.write_ack_pending {
                    self.write_ack_clock_high = true;
                } else {
                    trace!(
                        mode = ?self.mode,
                        next_bit = u8::from(sda_high),
                        bit_index = self.bit_count + 1,
                        "i2c sample bit"
                    );
                    self.shift = (self.shift << 1) | u8::from(sda_high);
                    self.bit_count += 1;
                    if self.bit_count == 8 {
                        let ack = self.handle_received_byte(analog, pcf8591, at24c02);
                        self.bit_count = 0;
                        self.shift = 0;
                        if ack {
                            self.write_ack_pending = true;
                            self.write_ack_clock_high = false;
                            self.sda_drive_low = true;
                        }
                    }
                }
            }
            I2cMode::Read => {
                if self.read_ack_pending {
                    self.read_ack_clock_high = true;
                    self.read_ack_master_high = master_sda_high;
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

    fn handle_scl_falling(
        &mut self,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) {
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
                if self.read_ack_master_high {
                    self.mode = I2cMode::Idle;
                    self.active = None;
                    self.read_buffer.clear();
                } else {
                    self.read_buffer.pop_front();
                    if self.read_buffer.is_empty() {
                        self.refill_read_buffer(analog, pcf8591, at24c02);
                    }
                }
            } else {
                self.sda_drive_low = false;
            }
        }
    }

    fn begin_start_condition(&mut self) {
        self.mode = I2cMode::Address;
        self.active = None;
        self.bit_count = 0;
        self.shift = 0;
        self.write_ack_pending = false;
        self.write_ack_clock_high = false;
        self.read_ack_pending = false;
        self.read_ack_clock_high = false;
        self.read_ack_master_high = true;
        self.enter_read_after_ack = false;
        self.read_buffer.clear();
        self.sda_drive_low = false;
    }

    fn finish_stop_condition(&mut self) {
        self.mode = I2cMode::Idle;
        self.active = None;
        self.bit_count = 0;
        self.shift = 0;
        self.write_ack_pending = false;
        self.write_ack_clock_high = false;
        self.read_ack_pending = false;
        self.read_ack_clock_high = false;
        self.read_ack_master_high = true;
        self.enter_read_after_ack = false;
        self.read_buffer.clear();
        self.sda_drive_low = false;
    }

    fn handle_received_byte(
        &mut self,
        analog: &AnalogInputs,
        pcf8591: &mut Pcf8591,
        at24c02: &mut At24c02,
    ) -> bool {
        match self.mode {
            I2cMode::Address => {
                let address = self.shift >> 1;
                let read = self.shift & 1 != 0;
                trace!(address, read, byte = self.shift, "i2c address");
                self.active = match address {
                    0x48 => Some(I2cDevice::Pcf8591),
                    0x50 => Some(I2cDevice::Eeprom),
                    _ => None,
                };

                match (self.active, read) {
                    (Some(I2cDevice::Pcf8591), true) => {
                        self.enter_read_after_ack = true;
                        self.read_buffer.push_back(pcf8591.read_byte(analog));
                        true
                    }
                    (Some(I2cDevice::Pcf8591), false) => {
                        pcf8591.begin_write();
                        self.mode = I2cMode::Write;
                        true
                    }
                    (Some(I2cDevice::Eeprom), true) => {
                        self.enter_read_after_ack = true;
                        self.read_buffer.push_back(at24c02.read_byte());
                        true
                    }
                    (Some(I2cDevice::Eeprom), false) => {
                        at24c02.begin_write();
                        self.mode = I2cMode::Write;
                        true
                    }
                    (None, _) => {
                        self.enter_read_after_ack = false;
                        self.mode = I2cMode::Idle;
                        false
                    }
                }
            }
            I2cMode::Write => match self.active {
                Some(I2cDevice::Pcf8591) => {
                    trace!(byte = self.shift, "i2c write pcf8591");
                    pcf8591.write_byte(self.shift);
                    true
                }
                Some(I2cDevice::Eeprom) => {
                    trace!(byte = self.shift, "i2c write eeprom");
                    at24c02.write_byte(self.shift);
                    true
                }
                None => false,
            },
            I2cMode::Idle | I2cMode::Read => false,
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

impl SclFilter {
    fn init(&mut self, time_ns: u64, raw_high: bool) {
        self.stable_high = raw_high;
        self.stable_since_ns = time_ns;
        self.raw_high = raw_high;
        self.raw_since_ns = time_ns;
    }

    fn observe(
        &mut self,
        time_ns: u64,
        raw_high: bool,
        rise_filter_ns: u64,
        fall_filter_ns: u64,
    ) -> Option<SclTransition> {
        if raw_high != self.raw_high {
            self.raw_high = raw_high;
            self.raw_since_ns = time_ns;
        }

        if self.raw_high == self.stable_high {
            return None;
        }

        let min_ns = if self.raw_high {
            rise_filter_ns
        } else {
            fall_filter_ns
        };
        if time_ns.saturating_sub(self.raw_since_ns) < min_ns {
            return None;
        }

        self.stable_high = self.raw_high;
        self.stable_since_ns = self.raw_since_ns.saturating_add(min_ns);
        Some(if self.stable_high {
            SclTransition::Rising
        } else {
            SclTransition::Falling
        })
    }

    fn high_stable_for_ns(&self, time_ns: u64) -> u64 {
        if self.stable_high {
            time_ns.saturating_sub(self.stable_since_ns)
        } else {
            0
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
        time_ns: u64,
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
                time_ns: 0,
            };
            harness.tick();
            harness
        }
    }

    impl I2cHarness {
        const STEP_NS: u64 = 10_000;

        fn tick(&mut self) {
            self.bus.sample(
                self.time_ns,
                self.scl,
                self.sda,
                &self.analog,
                &mut self.pcf8591,
                &mut self.at24c02,
            );
        }

        fn wait_ns(&mut self, ns: u64) {
            self.time_ns = self.time_ns.saturating_add(ns);
            self.tick();
        }

        fn wait_us(&mut self, us: u64) {
            self.wait_ns(us.saturating_mul(1_000));
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
            let (_, sda_high) = self.bus.line_levels(self.scl, self.sda);
            sda_high
        }

        fn start(&mut self) {
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
            self.set_scl(true);
            self.wait_ns(Self::STEP_NS);
            self.set_sda(false);
            self.wait_ns(Self::STEP_NS);
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
        }

        fn stop(&mut self) {
            self.set_sda(false);
            self.wait_ns(Self::STEP_NS);
            self.set_scl(true);
            self.wait_ns(Self::STEP_NS);
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
        }

        fn send_byte(&mut self, byte: u8) {
            let mut value = byte;
            for _ in 0..8 {
                self.set_scl(false);
                self.wait_ns(Self::STEP_NS);
                self.set_sda(value & 0x80 != 0);
                self.wait_ns(Self::STEP_NS);
                self.set_scl(true);
                self.wait_ns(Self::STEP_NS);
                value <<= 1;
            }
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
        }

        fn wait_ack(&mut self) -> bool {
            self.set_scl(true);
            self.wait_ns(Self::STEP_NS);
            let ack = self.line_sda_high();
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
            ack
        }

        fn receive_byte(&mut self) -> u8 {
            let mut value = 0_u8;
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
            for _ in 0..8 {
                self.set_scl(true);
                self.wait_ns(Self::STEP_NS);
                value = (value << 1) | u8::from(self.line_sda_high());
                self.set_scl(false);
                self.wait_ns(Self::STEP_NS);
            }
            value
        }

        fn send_ack(&mut self, ack: bool) {
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
            self.set_sda(ack);
            self.wait_ns(Self::STEP_NS);
            self.set_scl(true);
            self.wait_ns(Self::STEP_NS);
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
        }

        fn glitch_scl_low_us(&mut self, us: u64) {
            self.set_scl(false);
            self.wait_us(us);
            self.set_scl(true);
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

        fn eeprom_read_byte_with_ack_glitch(&mut self, addr: u8, glitch_us: u64) -> u8 {
            self.start();
            self.send_byte(0xA0);
            assert!(!self.wait_ack());
            self.send_byte(addr);
            assert!(!self.wait_ack());
            self.start();
            self.send_byte(0xA1);
            self.set_scl(true);
            self.wait_ns(Self::STEP_NS);
            assert!(!self.line_sda_high());
            self.glitch_scl_low_us(glitch_us);
            self.wait_ns(Self::STEP_NS);
            assert!(!self.line_sda_high());
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
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

    #[test]
    fn eeprom_random_read_ignores_short_scl_glitch_during_ack() {
        let mut harness = I2cHarness::default();
        harness.eeprom_write_byte(0x00, 0x01);
        harness.eeprom_write_byte(0x01, 0x02);

        assert_eq!(harness.eeprom_read_byte_with_ack_glitch(0x00, 1), 0x01);
        assert_eq!(harness.eeprom_read_byte_with_ack_glitch(0x01, 1), 0x02);
    }
}
