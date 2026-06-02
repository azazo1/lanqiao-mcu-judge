#[derive(Debug, Clone)]
pub(crate) struct At24c02 {
    memory: [u8; 256],
    address_pointer: u8,
    write_cursor: u8,
    current_page_base: u8,
    expecting_word_address: bool,
}

impl Default for At24c02 {
    fn default() -> Self {
        Self {
            memory: [0; 256],
            address_pointer: 0,
            write_cursor: 0,
            current_page_base: 0,
            expecting_word_address: true,
        }
    }
}

impl At24c02 {
    const PAGE_SIZE: u8 = 8;
    const PAGE_MASK: u8 = Self::PAGE_SIZE - 1;

    pub(crate) fn begin_write(&mut self) {
        self.expecting_word_address = true;
    }

    pub(crate) fn write_byte(&mut self, byte: u8) {
        if self.expecting_word_address {
            self.address_pointer = byte;
            self.write_cursor = byte;
            self.current_page_base = byte & !Self::PAGE_MASK;
            self.expecting_word_address = false;
            return;
        }

        self.memory[self.write_cursor as usize] = byte;

        let next_offset = self.write_cursor.wrapping_add(1) & Self::PAGE_MASK;
        self.write_cursor = self.current_page_base | next_offset;
        self.address_pointer = self.write_cursor;
    }

    pub(crate) fn read_byte(&mut self) -> u8 {
        let value = self.memory[self.address_pointer as usize];
        self.address_pointer = self.address_pointer.wrapping_add(1);
        value
    }
}

#[cfg(test)]
mod tests {
    use super::At24c02;

    #[test]
    fn writes_and_reads_sequential_bytes() {
        let mut eeprom = At24c02::default();
        eeprom.begin_write();
        eeprom.write_byte(0x10);
        eeprom.write_byte(0xAB);
        eeprom.write_byte(0xCD);

        eeprom.begin_write();
        eeprom.write_byte(0x10);
        assert_eq!(eeprom.read_byte(), 0xAB);
        assert_eq!(eeprom.read_byte(), 0xCD);
    }

    #[test]
    fn page_write_rolls_over_inside_same_page() {
        let mut eeprom = At24c02::default();
        eeprom.begin_write();
        eeprom.write_byte(0x0E);
        eeprom.write_byte(0xA0);
        eeprom.write_byte(0xB1);
        eeprom.write_byte(0xC2);

        eeprom.begin_write();
        eeprom.write_byte(0x08);
        assert_eq!(eeprom.read_byte(), 0xC2);
        assert_eq!(eeprom.read_byte(), 0x00);
        assert_eq!(eeprom.read_byte(), 0x00);
        assert_eq!(eeprom.read_byte(), 0x00);
        assert_eq!(eeprom.read_byte(), 0x00);
        assert_eq!(eeprom.read_byte(), 0x00);
        assert_eq!(eeprom.read_byte(), 0xA0);
        assert_eq!(eeprom.read_byte(), 0xB1);
    }
}
