use anyhow::Result;

use crate::ids::KeyId;

#[derive(Debug, Default, Clone)]
pub(crate) struct Key {
    pressed: [[bool; 4]; 4],
    pressed_count: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct KeyboardLows {
    pub(crate) rows: [bool; 4],
    pub(crate) cols: [bool; 4],
}

impl Key {
    pub(crate) fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.set_key_id(KeyId::parse(name)?, pressed);
        Ok(())
    }

    pub(crate) fn set_key_id(&mut self, key: KeyId, pressed: bool) {
        let (row, col) = key.matrix_position();
        let old = self.pressed[row][col];
        if old != pressed {
            if pressed {
                self.pressed_count += 1;
            } else {
                self.pressed_count -= 1;
            }
        }
        self.pressed[row][col] = pressed;
    }

    pub(crate) fn pressed(&self, key: KeyId) -> bool {
        let (row, col) = key.matrix_position();
        self.pressed[row][col]
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn row_low(&self, row: u8, latches: &[u8; 6]) -> bool {
        if self.pressed_count == 0 {
            return false;
        }
        for col in 0..4 {
            if self.pressed[row as usize][col] && self.column_driven_low(col, latches) {
                return true;
            }
        }
        false
    }

    pub(crate) fn col_low(&self, col: u8, latches: &[u8; 6]) -> bool {
        if self.pressed_count == 0 {
            return false;
        }
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

    pub(crate) fn button_row_lows(&self) -> [bool; 4] {
        if self.pressed_count == 0 {
            return [false; 4];
        }
        [
            self.button_row_low(0),
            self.button_row_low(1),
            self.button_row_low(2),
            self.button_row_low(3),
        ]
    }

    pub(crate) fn keyboard_lows(&self, latches: &[u8; 6]) -> KeyboardLows {
        if self.pressed_count == 0 {
            return KeyboardLows::default();
        }
        let row_driven_low = [
            self.row_driven_low(0, latches),
            self.row_driven_low(1, latches),
            self.row_driven_low(2, latches),
            self.row_driven_low(3, latches),
        ];
        let col_driven_low = [
            self.column_driven_low(0, latches),
            self.column_driven_low(1, latches),
            self.column_driven_low(2, latches),
            self.column_driven_low(3, latches),
        ];
        let mut lows = KeyboardLows::default();
        for (row, row_is_driven_low) in row_driven_low.iter().copied().enumerate() {
            for (col, col_is_driven_low) in col_driven_low.iter().copied().enumerate() {
                if !self.pressed[row][col] {
                    continue;
                }
                if col_is_driven_low {
                    lows.rows[row] = true;
                }
                if row_is_driven_low {
                    lows.cols[col] = true;
                }
            }
        }
        lows
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

#[cfg(test)]
mod tests {
    use super::Key;
    use crate::ids::KeyId;

    #[test]
    fn keyboard_lows_match_individual_row_and_col_queries() {
        let mut key = Key::default();
        key.set_key_id(KeyId::S4, true);
        key.set_key_id(KeyId::S11, true);
        key.set_key_id(KeyId::S19, true);

        let mut latches = [0xFF; 6];
        latches[3] &= !(1 << 2);
        latches[4] &= !(1 << 4);

        let lows = key.keyboard_lows(&latches);
        for row in 0..4 {
            assert_eq!(lows.rows[row], key.row_low(row as u8, &latches));
            assert_eq!(lows.cols[row], key.col_low(row as u8, &latches));
        }
    }
}
