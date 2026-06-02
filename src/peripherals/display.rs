use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};

#[derive(Debug, Default)]
pub(crate) struct Outputs {
    pub(crate) leds: [bool; 8],
    pub(crate) relay_on: bool,
    pub(crate) motor_on: bool,
    pub(crate) buzzer_on: bool,
    pub(crate) digits: [DigitSample; 8],
    segment_latch: u8,
    com_latch: u8,
    pending_com_latch: u8,
    last_com_strobe: u64,
    last_seg_strobe: u64,
}

impl Outputs {
    pub(crate) fn sample_from_latches(&mut self, latches: &[u8; 4], versions: &[u64; 4]) {
        let led = latches[0];
        for bit in 0..8 {
            self.leds[bit] = led & (1 << bit) == 0;
        }

        let ctrl = latches[1];
        self.relay_on = ctrl & (1 << 4) != 0;
        self.motor_on = ctrl & (1 << 5) != 0;
        self.buzzer_on = ctrl & (1 << 6) != 0;

        if versions[2] != self.last_com_strobe {
            self.com_latch = latches[2];
            self.pending_com_latch = self.com_latch;
            self.last_com_strobe = versions[2];
        }

        if versions[3] != self.last_seg_strobe {
            self.segment_latch = latches[3];
            self.last_seg_strobe = versions[3];

            if self.pending_com_latch != 0 {
                for digit in 0..8 {
                    if self.pending_com_latch & (1 << digit) != 0 {
                        self.digits[digit].segments = self.segment_latch;
                        self.digits[digit].seen = true;
                    }
                }
                self.pending_com_latch = 0;
            }
        }
    }

    pub(crate) fn display_text(&self, decoder: &SegmentDecoder) -> String {
        self.digits
            .iter()
            .map(|digit| decoder.decode_char(*digit))
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    pub(crate) fn seg_raw(&self, index: usize) -> Result<u8> {
        let Some(digit) = self.digits.get(index.saturating_sub(1)) else {
            bail!("数码管编号必须在 1..=8");
        };
        if !digit.seen {
            bail!("数码管 D{index} 尚未采样到有效段码");
        }
        Ok(digit.segments)
    }

    pub(crate) fn seg_pattern(&self, index: usize) -> Result<u8> {
        Ok(!self.seg_raw(index)?)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DigitSample {
    pub(crate) segments: u8,
    pub(crate) seen: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SegmentDecoder {
    char_map: BTreeMap<u8, char>,
    blank_patterns: BTreeSet<u8>,
}

impl Default for SegmentDecoder {
    fn default() -> Self {
        let mut decoder = Self {
            char_map: BTreeMap::new(),
            blank_patterns: BTreeSet::from([0x00]),
        };
        for (pattern, ch) in [
            (0x3F, '0'),
            (0x06, '1'),
            (0x5B, '2'),
            (0x4F, '3'),
            (0x66, '4'),
            (0x6D, '5'),
            (0x7D, '6'),
            (0x07, '7'),
            (0x7F, '8'),
            (0x6F, '9'),
            (0x40, '-'),
            (0x73, 'P'),
            (0x79, 'E'),
            (0x38, 'L'),
            (0x71, 'F'),
            (0x76, 'H'),
            (0x39, 'C'),
        ] {
            decoder.char_map.insert(pattern, ch);
        }
        decoder
    }
}

impl SegmentDecoder {
    pub(crate) fn set_mapping(&mut self, pattern: u8, text: &str) -> Result<()> {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (None, None) => {
                self.char_map.remove(&pattern);
                self.blank_patterns.insert(pattern);
                Ok(())
            }
            (Some(ch), None) => {
                self.blank_patterns.remove(&pattern);
                self.char_map.insert(pattern, ch);
                Ok(())
            }
            _ => bail!("set_seg_decode 只接受空串或单个字符"),
        }
    }

    pub(crate) fn mark_blank(&mut self, pattern: u8) {
        self.char_map.remove(&pattern);
        self.blank_patterns.insert(pattern);
    }

    pub(crate) fn decode_char(&self, digit: DigitSample) -> char {
        if !digit.seen {
            return ' ';
        }
        let pattern = !digit.segments;
        if self.blank_patterns.contains(&pattern) {
            return ' ';
        }
        self.char_map
            .get(&(pattern & 0x7F))
            .copied()
            .unwrap_or(if pattern & 0x80 != 0 { '.' } else { '?' })
    }
}
