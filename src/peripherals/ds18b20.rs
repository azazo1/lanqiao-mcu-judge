use std::collections::VecDeque;

use crate::timing::CPU_TICKS_PER_US;

#[derive(Debug, Default)]
pub(crate) struct Ds18b20 {
    pub(crate) drive_low: bool,
    pub(crate) temperature_c: f32,
    line_prev: bool,
    low_since: Option<u64>,
    awaiting_command: bool,
    command_bits: u8,
    command_value: u8,
    output_bits: VecDeque<bool>,
    read_slot_until: Option<u64>,
}

impl Ds18b20 {
    pub(crate) fn sample(&mut self, ticks: u64, line_high: bool) {
        if !line_high && self.line_prev {
            self.low_since = Some(ticks);
        }

        if line_high
            && !self.line_prev
            && let Some(start) = self.low_since.take()
        {
            let width = ticks.saturating_sub(start);
            if width >= 400 * CPU_TICKS_PER_US {
                self.drive_low = false;
                self.awaiting_command = true;
                self.command_bits = 0;
                self.command_value = 0;
                self.read_slot_until = Some(ticks + 120 * CPU_TICKS_PER_US);
                self.drive_low = true;
            } else if self.awaiting_command {
                let bit = width < 15 * CPU_TICKS_PER_US;
                if bit {
                    self.command_value |= 1 << self.command_bits;
                }
                self.command_bits += 1;
                if self.command_bits == 8 {
                    self.handle_command();
                    self.command_bits = 0;
                    self.command_value = 0;
                }
            } else if let Some(bit) = self.output_bits.pop_front() {
                self.drive_low = !bit;
                self.read_slot_until = Some(ticks + 45 * CPU_TICKS_PER_US);
            }
        }

        if let Some(until) = self.read_slot_until
            && ticks >= until
        {
            self.drive_low = false;
            self.read_slot_until = None;
        }

        self.line_prev = line_high;
    }

    fn handle_command(&mut self) {
        match self.command_value {
            0xCC => {
                self.awaiting_command = true;
            }
            0x44 => {
                self.awaiting_command = false;
            }
            0xBE => {
                self.awaiting_command = false;
                let bytes = self.scratchpad();
                self.output_bits.clear();
                for byte in bytes {
                    for bit in 0..8 {
                        self.output_bits.push_back(byte & (1 << bit) != 0);
                    }
                }
            }
            _ => {
                self.awaiting_command = false;
            }
        }
    }

    fn scratchpad(&self) -> [u8; 9] {
        let raw = (self.temperature_c * 16.0).round() as i16;
        [
            raw as u8,
            (raw >> 8) as u8,
            0x4B,
            0x46,
            0x7F,
            0xFF,
            0x0C,
            0x10,
            0x00,
        ]
    }
}
