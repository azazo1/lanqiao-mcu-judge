use std::mem;

use crate::persistent_state::At24c02PersistentState;

use super::i2c_slave::{I2cSlaveDevice, I2cSlaveFrontend, I2cSlaveTiming};

#[derive(Debug, Clone)]
pub(crate) struct At24c02 {
    memory: [u8; 256],
    address_pointer: u8,
    write_cursor: u8,
    current_page_base: u8,
    expecting_word_address: bool,
    page_shadow: [u8; Self::PAGE_SIZE as usize],
    page_dirty_mask: u8,
    write_cycle_until_ns: u64,
    frontend: I2cSlaveFrontend,
}

impl Default for At24c02 {
    fn default() -> Self {
        Self {
            memory: [0; 256],
            address_pointer: 0,
            write_cursor: 0,
            current_page_base: 0,
            expecting_word_address: true,
            page_shadow: [0; Self::PAGE_SIZE as usize],
            page_dirty_mask: 0,
            write_cycle_until_ns: 0,
            frontend: I2cSlaveFrontend::default(),
        }
    }
}

impl At24c02 {
    const ADDRESS7: u8 = 0x50;
    const PAGE_SIZE: u8 = 8;
    const PAGE_MASK: u8 = Self::PAGE_SIZE - 1;
    const WRITE_CYCLE_NS: u64 = 5_000_000;
    const TIMING: I2cSlaveTiming = I2cSlaveTiming {
        min_scl_low_ns: 1_300,
        min_scl_high_ns: 700,
        min_start_stop_scl_high_ns: 700,
    };

    pub(crate) fn byte(&self, addr: u8) -> u8 {
        self.memory[addr as usize]
    }

    pub(crate) fn persistent_state(&self) -> At24c02PersistentState {
        At24c02PersistentState {
            memory: self.memory,
        }
    }

    pub(crate) fn load_persistent_state(&mut self, state: &At24c02PersistentState) {
        self.memory = state.memory;
        self.page_shadow = [0; Self::PAGE_SIZE as usize];
        self.page_dirty_mask = 0;
        self.write_cycle_until_ns = 0;
    }

    pub(crate) fn sample_i2c(&mut self, time_ns: u64, scl_high: bool, sda_high: bool) {
        let mut frontend = mem::take(&mut self.frontend);
        frontend.sample(time_ns, scl_high, sda_high, self, &());
        self.frontend = frontend;
    }

    pub(crate) fn sda_drive_low(&self) -> bool {
        self.frontend.sda_drive_low()
    }

    pub(crate) fn scl_drive_low(&self) -> bool {
        self.frontend.scl_drive_low()
    }

    pub(crate) fn settle_i2c_lines(&mut self, sda_high: bool) {
        self.frontend.settle_lines(sda_high);
    }

    fn busy(&self, time_ns: u64) -> bool {
        time_ns < self.write_cycle_until_ns
    }

    fn begin_write_transaction(&mut self) {
        self.expecting_word_address = true;
        self.page_dirty_mask = 0;
    }

    fn buffer_write_byte(&mut self, byte: u8) {
        if self.expecting_word_address {
            self.address_pointer = byte;
            self.write_cursor = byte;
            self.current_page_base = byte & !Self::PAGE_MASK;
            for offset in 0..Self::PAGE_SIZE {
                let addr = self.current_page_base | offset;
                self.page_shadow[offset as usize] = self.memory[addr as usize];
            }
            self.expecting_word_address = false;
            self.page_dirty_mask = 0;
            return;
        }

        let offset = self.write_cursor & Self::PAGE_MASK;
        self.page_shadow[offset as usize] = byte;
        self.page_dirty_mask |= 1 << offset;

        let next_offset = self.write_cursor.wrapping_add(1) & Self::PAGE_MASK;
        self.write_cursor = self.current_page_base | next_offset;
        self.address_pointer = self.write_cursor;
    }

    fn commit_page_write(&mut self, time_ns: u64) {
        if self.page_dirty_mask == 0 {
            self.expecting_word_address = true;
            return;
        }

        for offset in 0..Self::PAGE_SIZE {
            if self.page_dirty_mask & (1 << offset) == 0 {
                continue;
            }
            let addr = self.current_page_base | offset;
            self.memory[addr as usize] = self.page_shadow[offset as usize];
        }

        self.page_dirty_mask = 0;
        self.expecting_word_address = true;
        self.write_cycle_until_ns = time_ns.saturating_add(Self::WRITE_CYCLE_NS);
    }

    fn read_current_byte(&mut self) -> u8 {
        let value = self.memory[self.address_pointer as usize];
        self.address_pointer = self.address_pointer.wrapping_add(1);
        value
    }
}

impl I2cSlaveDevice for At24c02 {
    type Context = ();

    fn address7(&self) -> u8 {
        Self::ADDRESS7
    }

    fn timing(&self) -> I2cSlaveTiming {
        Self::TIMING
    }

    fn on_i2c_stop(&mut self, time_ns: u64, _ctx: &Self::Context) {
        self.commit_page_write(time_ns);
    }

    fn on_addressed_write(&mut self, time_ns: u64, _ctx: &Self::Context) -> bool {
        if self.busy(time_ns) {
            return false;
        }
        self.begin_write_transaction();
        true
    }

    fn on_addressed_read(&mut self, time_ns: u64, _ctx: &Self::Context) -> Option<u8> {
        if self.busy(time_ns) {
            return None;
        }
        Some(self.read_current_byte())
    }

    fn on_write_byte(&mut self, time_ns: u64, byte: u8, _ctx: &Self::Context) -> bool {
        if self.busy(time_ns) {
            return false;
        }
        self.buffer_write_byte(byte);
        true
    }

    fn on_read_continue(&mut self, time_ns: u64, _ctx: &Self::Context) -> u8 {
        if self.busy(time_ns) {
            return 0xFF;
        }
        self.read_current_byte()
    }
}

#[cfg(test)]
mod tests {
    use super::At24c02;
    use crate::peripherals::i2c_slave::I2cSlaveDevice;

    #[test]
    fn page_write_rolls_over_inside_same_page() {
        let mut eeprom = At24c02::default();
        eeprom.begin_write_transaction();
        eeprom.buffer_write_byte(0x0E);
        eeprom.buffer_write_byte(0xA0);
        eeprom.buffer_write_byte(0xB1);
        eeprom.buffer_write_byte(0xC2);
        eeprom.commit_page_write(0);
        eeprom.write_cycle_until_ns = 0;

        eeprom.address_pointer = 0x08;
        assert_eq!(eeprom.read_current_byte(), 0xC2);
        assert_eq!(eeprom.read_current_byte(), 0x00);
        assert_eq!(eeprom.read_current_byte(), 0x00);
        assert_eq!(eeprom.read_current_byte(), 0x00);
        assert_eq!(eeprom.read_current_byte(), 0x00);
        assert_eq!(eeprom.read_current_byte(), 0x00);
        assert_eq!(eeprom.read_current_byte(), 0xA0);
        assert_eq!(eeprom.read_current_byte(), 0xB1);
    }

    #[test]
    fn stop_starts_busy_write_cycle() {
        let mut eeprom = At24c02::default();
        eeprom.begin_write_transaction();
        eeprom.buffer_write_byte(0x10);
        eeprom.buffer_write_byte(0xAB);
        eeprom.commit_page_write(12_345);

        assert_eq!(eeprom.byte(0x10), 0xAB);
        assert!(eeprom.busy(12_345));
        assert!(eeprom.busy(12_345 + At24c02::WRITE_CYCLE_NS - 1));
        assert!(!eeprom.busy(12_345 + At24c02::WRITE_CYCLE_NS));
    }

    #[test]
    fn busy_cycle_nacks_address_until_write_cycle_finishes() {
        let mut eeprom = At24c02::default();
        eeprom.begin_write_transaction();
        eeprom.buffer_write_byte(0x10);
        eeprom.buffer_write_byte(0xAB);
        eeprom.commit_page_write(50);

        assert!(!<At24c02 as I2cSlaveDevice>::on_addressed_write(&mut eeprom, 50, &()));
        assert!(<At24c02 as I2cSlaveDevice>::on_addressed_read(&mut eeprom, 50, &()).is_none());
        assert!(<At24c02 as I2cSlaveDevice>::on_addressed_write(
            &mut eeprom,
            50 + At24c02::WRITE_CYCLE_NS,
            &()
        ));
    }
}
