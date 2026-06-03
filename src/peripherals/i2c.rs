use super::{AnalogInputs, At24c02, Pcf8591};

#[derive(Debug, Default)]
pub(crate) struct I2cBus;

impl I2cBus {
    pub(crate) fn line_levels(
        &self,
        master_scl_high: bool,
        master_sda_high: bool,
        slave_scl_low: bool,
        slave_sda_low: bool,
    ) -> (bool, bool) {
        (master_scl_high && !slave_scl_low, master_sda_high && !slave_sda_low)
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
        let (slave_scl_low, slave_sda_low) = self.slave_drives_low(pcf8591, at24c02);
        let (scl_high, sda_high) =
            self.line_levels(master_scl_high, master_sda_high, slave_scl_low, slave_sda_low);

        pcf8591.sample_i2c(time_ns, scl_high, sda_high, analog);
        at24c02.sample_i2c(time_ns, scl_high, sda_high);

        let (slave_scl_low, slave_sda_low) = self.slave_drives_low(pcf8591, at24c02);
        let (_, settled_sda_high) =
            self.line_levels(master_scl_high, master_sda_high, slave_scl_low, slave_sda_low);
        pcf8591.settle_i2c_lines(settled_sda_high);
        at24c02.settle_i2c_lines(settled_sda_high);
    }

    pub(crate) fn slave_drives_low(&self, pcf8591: &Pcf8591, at24c02: &At24c02) -> (bool, bool) {
        (
            pcf8591.scl_drive_low() || at24c02.scl_drive_low(),
            pcf8591.sda_drive_low() || at24c02.sda_drive_low(),
        )
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
                bus: I2cBus,
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
            let (slave_scl_low, slave_sda_low) = self.bus.slave_drives_low(&self.pcf8591, &self.at24c02);
            let (_, sda_high) = self
                .bus
                .line_levels(self.scl, self.sda, slave_scl_low, slave_sda_low);
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
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
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

        fn send_ack(&mut self, nack: bool) {
            self.set_scl(false);
            self.wait_ns(Self::STEP_NS);
            self.set_sda(nack);
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
            self.set_sda(true);
            self.wait_ns(Self::STEP_NS);
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
        harness.wait_us(5_100);
        harness.eeprom_write_byte(0x01, 0x02);
        harness.wait_us(5_100);
        harness.eeprom_write_byte(0x02, 0x03);
        harness.wait_us(5_100);
        harness.eeprom_write_byte(0x20, 0x09);
        harness.wait_us(5_100);

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
        harness.wait_us(5_100);
        harness.eeprom_write_byte(0x01, 0x02);
        harness.wait_us(5_100);

        assert_eq!(harness.eeprom_read_byte_with_ack_glitch(0x00, 1), 0x01);
        assert_eq!(harness.eeprom_read_byte_with_ack_glitch(0x01, 1), 0x02);
    }
}
