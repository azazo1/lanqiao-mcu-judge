use anyhow::Result;

use crate::ids::KeyId;

#[derive(Debug, Default, Clone)]
pub(crate) struct Key {
    pressed: [[bool; 4]; 4],
}

impl Key {
    pub(crate) fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.set_key_id(KeyId::parse(name)?, pressed);
        Ok(())
    }

    pub(crate) fn set_key_id(&mut self, key: KeyId, pressed: bool) {
        let (row, col) = key.matrix_position();
        self.pressed[row][col] = pressed;
    }

    pub(crate) fn pressed(&self, key: KeyId) -> bool {
        let (row, col) = key.matrix_position();
        self.pressed[row][col]
    }

    pub(crate) fn row_low(&self, row: u8, latches: &[u8; 6]) -> bool {
        for col in 0..4 {
            if self.pressed[row as usize][col] && self.column_driven_low(col, latches) {
                return true;
            }
        }
        false
    }

    pub(crate) fn col_low(&self, col: u8, latches: &[u8; 6]) -> bool {
        for row in 0..4 {
            if self.pressed[row][col as usize] && self.row_driven_low(row, latches) {
                return true;
            }
        }
        false
    }

    pub(crate) fn button_row_low(&self, row: u8) -> bool {
        self.pressed[row as usize][0]
    }

    fn row_driven_low(&self, row: usize, latches: &[u8; 6]) -> bool {
        let p3 = latches[3];
        match row {
            0 => p3 & (1 << 0) == 0,
            1 => p3 & (1 << 1) == 0,
            2 => p3 & (1 << 2) == 0,
            3 => p3 & (1 << 3) == 0,
            _ => false,
        }
    }

    fn column_driven_low(&self, col: usize, latches: &[u8; 6]) -> bool {
        match col {
            0 => latches[4] & (1 << 4) == 0,
            1 => latches[4] & (1 << 2) == 0,
            2 => latches[3] & (1 << 5) == 0,
            3 => latches[3] & (1 << 4) == 0,
            _ => false,
        }
    }
}
