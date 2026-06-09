use crate::wave::{TRACK_EVENT_I2C, WaveEventNote};

#[derive(Debug, Default, Clone)]
pub(crate) struct I2cEventDecoder {
    initialized: bool,
    prev_scl: bool,
    prev_sda: bool,
    active: bool,
    bit_count: u8,
    shift: u8,
    waiting_ack: bool,
    expecting_address: bool,
    reading: bool,
    last_byte: u8,
}

impl I2cEventDecoder {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn observe(
        &mut self,
        time_ns: u64,
        scl_high: bool,
        sda_high: bool,
    ) -> Vec<WaveEventNote> {
        let mut events = Vec::new();
        if !self.initialized {
            self.initialized = true;
            self.prev_scl = scl_high;
            self.prev_sda = sda_high;
            return events;
        }

        let start = self.prev_sda && !sda_high && self.prev_scl && scl_high;
        let stop = !self.prev_sda && sda_high && self.prev_scl && scl_high;

        if start {
            let label = if self.active {
                "REPEATED START"
            } else {
                "START"
            };
            events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, label));
            self.active = true;
            self.bit_count = 0;
            self.shift = 0;
            self.waiting_ack = false;
            self.expecting_address = true;
        }

        if self.active && !self.prev_scl && scl_high {
            if self.waiting_ack {
                let ack = !sda_high;
                let label = if ack { "ACK" } else { "NACK" };
                events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, label));
                self.waiting_ack = false;
                if self.expecting_address {
                    self.reading = self.last_byte & 0x01 != 0;
                    self.expecting_address = false;
                }
            } else {
                self.shift = (self.shift << 1) | u8::from(sda_high);
                self.bit_count += 1;
                if self.bit_count == 8 {
                    let byte = self.shift;
                    let note = if self.expecting_address {
                        WaveEventNote::with_detail(
                            time_ns,
                            TRACK_EVENT_I2C,
                            format!(
                                "ADDR 0x{:02X} {}",
                                byte,
                                if byte & 0x01 != 0 { "R" } else { "W" }
                            ),
                            format!("raw=0x{byte:02X}"),
                        )
                    } else if self.reading {
                        WaveEventNote::new(time_ns, TRACK_EVENT_I2C, format!("RX 0x{byte:02X}"))
                    } else {
                        WaveEventNote::new(time_ns, TRACK_EVENT_I2C, format!("TX 0x{byte:02X}"))
                    };
                    events.push(note);
                    self.last_byte = byte;
                    self.shift = 0;
                    self.bit_count = 0;
                    self.waiting_ack = true;
                }
            }
        }

        if stop && self.active {
            events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, "STOP"));
            self.active = false;
            self.bit_count = 0;
            self.shift = 0;
            self.waiting_ack = false;
            self.expecting_address = false;
            self.reading = false;
        }

        self.prev_scl = scl_high;
        self.prev_sda = sda_high;
        events
    }
}

#[cfg(test)]
mod tests {
    use crate::wave::TRACK_EVENT_I2C;

    use super::I2cEventDecoder;

    #[test]
    fn i2c_decoder_marks_start_bytes_ack_and_stop() {
        let mut decoder = I2cEventDecoder::default();
        let mut labels = Vec::new();

        let mut samples = vec![(0, true, true), (10, true, false)];
        let mut time_ns = 20;
        for bit in [true, false, true, false, false, false, false, false] {
            samples.push((time_ns, false, bit));
            time_ns += 10;
            samples.push((time_ns, true, bit));
            time_ns += 10;
        }
        samples.push((time_ns, false, false));
        time_ns += 10;
        samples.push((time_ns, true, false));
        time_ns += 10;
        samples.push((time_ns, true, true));

        for (time_ns, scl, sda) in samples {
            for event in decoder.observe(time_ns, scl, sda) {
                assert_eq!(event.track_id, TRACK_EVENT_I2C);
                labels.push(event.label);
            }
        }

        assert_eq!(labels, ["START", "ADDR 0xA0 W", "ACK", "STOP"]);
    }

    #[test]
    fn i2c_decoder_resets_cleanly_after_pause() {
        let mut decoder = I2cEventDecoder::default();
        assert!(decoder.observe(0, true, true).is_empty());
        decoder.reset();
        assert!(decoder.observe(10, false, false).is_empty());
    }
}
