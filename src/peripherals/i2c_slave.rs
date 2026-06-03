use tracing::trace;

#[derive(Debug, Clone, Copy)]
pub(crate) struct I2cSlaveTiming {
    pub(crate) min_scl_low_ns: u64,
    pub(crate) min_scl_high_ns: u64,
    pub(crate) min_start_stop_scl_high_ns: u64,
}

pub(crate) trait I2cSlaveDevice {
    type Context;

    fn address7(&self) -> u8;

    fn timing(&self) -> I2cSlaveTiming;

    fn on_i2c_start(&mut self, _time_ns: u64, _ctx: &Self::Context) {}

    fn on_i2c_stop(&mut self, _time_ns: u64, _ctx: &Self::Context) {}

    fn on_addressed_write(&mut self, _time_ns: u64, _ctx: &Self::Context) -> bool;

    fn on_addressed_read(&mut self, time_ns: u64, ctx: &Self::Context) -> Option<u8>;

    fn on_write_byte(&mut self, time_ns: u64, byte: u8, ctx: &Self::Context) -> bool;

    fn on_read_continue(&mut self, time_ns: u64, ctx: &Self::Context) -> u8;

    fn on_read_finished(&mut self, _time_ns: u64, _ctx: &Self::Context, _master_nack: bool) {}
}

#[derive(Debug, Clone, Default)]
pub(crate) struct I2cSlaveFrontend {
    initialized: bool,
    scl_filter: SclFilter,
    line_sda_prev: bool,
    bit_count: u8,
    shift: u8,
    write_ack_pending: bool,
    write_ack_clock_high: bool,
    read_ack_pending: bool,
    read_ack_clock_high: bool,
    read_ack_master_high: bool,
    sda_drive_low: bool,
    scl_drive_low: bool,
    mode: I2cMode,
    enter_read_after_ack: bool,
}

#[derive(Debug, Clone, Default)]
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

impl I2cSlaveFrontend {
    pub(crate) fn sda_drive_low(&self) -> bool {
        self.sda_drive_low
    }

    pub(crate) fn scl_drive_low(&self) -> bool {
        self.scl_drive_low
    }

    pub(crate) fn settle_lines(&mut self, sda_high: bool) {
        self.line_sda_prev = sda_high;
    }

    #[cfg(test)]
    pub(crate) fn force_drive_levels(&mut self, scl_low: bool, sda_low: bool) {
        self.scl_drive_low = scl_low;
        self.sda_drive_low = sda_low;
    }

    pub(crate) fn sample<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        scl_high: bool,
        sda_high: bool,
        device: &mut D,
        ctx: &D::Context,
    ) {
        let timing = device.timing();
        if !self.initialized {
            self.initialized = true;
            self.line_sda_prev = sda_high;
            self.scl_filter.init(time_ns, scl_high);
            return;
        }

        let raw_scl_prev = self.scl_filter.raw_high;
        if scl_high != raw_scl_prev {
            trace!(
                time_ns,
                address = device.address7(),
                raw_scl = scl_high,
                "i2c slave raw scl"
            );
        }
        if self.line_sda_prev != sda_high {
            trace!(
                time_ns,
                address = device.address7(),
                raw_sda = sda_high,
                "i2c slave raw sda"
            );
        }

        if !raw_scl_prev && scl_high {
            if self.write_ack_pending {
                self.write_ack_clock_high = true;
                self.sda_drive_low = true;
            }
            if self.read_ack_pending {
                self.read_ack_clock_high = true;
                self.read_ack_master_high = sda_high;
            }
        }
        if raw_scl_prev && !scl_high && self.write_ack_pending && self.write_ack_clock_high {
            self.sda_drive_low = false;
        }

        let rise_filter_ns = if self.write_ack_pending || self.read_ack_pending {
            0
        } else {
            timing.min_scl_high_ns
        };
        if let Some(transition) = self.scl_filter.observe(
            time_ns,
            scl_high,
            rise_filter_ns,
            timing.min_scl_low_ns,
        ) {
            match transition {
                SclTransition::Rising => {
                    self.handle_scl_rising(time_ns, sda_high, device, ctx);
                }
                SclTransition::Falling => self.handle_scl_falling(time_ns, device, ctx),
            }
        }

        if self.line_sda_prev != sda_high
            && self.scl_filter.high_stable_for_ns(time_ns) >= timing.min_start_stop_scl_high_ns
        {
            if self.line_sda_prev && !sda_high {
                trace!(time_ns, address = device.address7(), "i2c slave start");
                self.begin_start_condition(time_ns, device, ctx);
            } else if !self.line_sda_prev && sda_high {
                trace!(time_ns, address = device.address7(), "i2c slave stop");
                self.finish_stop_condition(time_ns, device, ctx);
            }
        }
    }

    fn handle_scl_rising<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        sda_high: bool,
        device: &mut D,
        ctx: &D::Context,
    ) {
        match self.mode {
            I2cMode::Address | I2cMode::Write => {
                if self.write_ack_pending {
                    self.write_ack_clock_high = true;
                } else {
                    trace!(
                        time_ns,
                        address = device.address7(),
                        mode = ?self.mode,
                        next_bit = u8::from(sda_high),
                        bit_index = self.bit_count + 1,
                        "i2c slave sample bit"
                    );
                    self.shift = (self.shift << 1) | u8::from(sda_high);
                    self.bit_count += 1;
                    if self.bit_count == 8 {
                        let ack = self.handle_received_byte(time_ns, device, ctx);
                        self.bit_count = 0;
                        if !(ack && self.enter_read_after_ack) {
                            self.shift = 0;
                        }
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
                    self.read_ack_master_high = sda_high;
                } else {
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

    fn handle_scl_falling<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        device: &mut D,
        ctx: &D::Context,
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
                    device.on_read_finished(time_ns, ctx, true);
                } else {
                    self.shift = device.on_read_continue(time_ns, ctx);
                    device.on_read_finished(time_ns, ctx, false);
                }
            } else {
                self.sda_drive_low = false;
            }
        }
    }

    fn handle_received_byte<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        device: &mut D,
        ctx: &D::Context,
    ) -> bool {
        match self.mode {
            I2cMode::Address => {
                let address = self.shift >> 1;
                let read = self.shift & 1 != 0;
                trace!(
                    time_ns,
                    listener = device.address7(),
                    address,
                    read,
                    byte = self.shift,
                    "i2c slave address"
                );
                if address != device.address7() {
                    self.enter_read_after_ack = false;
                    self.mode = I2cMode::Idle;
                    return false;
                }

                if read {
                    let Some(byte) = device.on_addressed_read(time_ns, ctx) else {
                        self.enter_read_after_ack = false;
                        self.mode = I2cMode::Idle;
                        return false;
                    };
                    self.shift = byte;
                    self.enter_read_after_ack = true;
                    true
                } else {
                    let ack = device.on_addressed_write(time_ns, ctx);
                    if ack {
                        self.mode = I2cMode::Write;
                    } else {
                        self.mode = I2cMode::Idle;
                    }
                    ack
                }
            }
            I2cMode::Write => device.on_write_byte(time_ns, self.shift, ctx),
            I2cMode::Idle | I2cMode::Read => false,
        }
    }

    fn begin_start_condition<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        device: &mut D,
        ctx: &D::Context,
    ) {
        self.mode = I2cMode::Address;
        self.bit_count = 0;
        self.shift = 0;
        self.write_ack_pending = false;
        self.write_ack_clock_high = false;
        self.read_ack_pending = false;
        self.read_ack_clock_high = false;
        self.read_ack_master_high = true;
        self.enter_read_after_ack = false;
        self.sda_drive_low = false;
        device.on_i2c_start(time_ns, ctx);
    }

    fn finish_stop_condition<D: I2cSlaveDevice>(
        &mut self,
        time_ns: u64,
        device: &mut D,
        ctx: &D::Context,
    ) {
        self.mode = I2cMode::Idle;
        self.bit_count = 0;
        self.shift = 0;
        self.write_ack_pending = false;
        self.write_ack_clock_high = false;
        self.read_ack_pending = false;
        self.read_ack_clock_high = false;
        self.read_ack_master_high = true;
        self.enter_read_after_ack = false;
        self.sda_drive_low = false;
        device.on_i2c_stop(time_ns, ctx);
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
