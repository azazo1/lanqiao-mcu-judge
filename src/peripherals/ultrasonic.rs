use std::collections::VecDeque;

use crate::chip::NS_PER_MICROSECOND;

#[derive(Debug)]
pub(crate) struct UltrasonicDevice {
    pub(crate) distance_cm: f32,
    pending_rx: VecDeque<u8>,
    tx_prev_high: bool,
    waiting_for_trigger_release: bool,
    rx_high: bool,
    echo_ns_remaining: Option<u64>,
}

impl Default for UltrasonicDevice {
    fn default() -> Self {
        Self {
            distance_cm: 0.0,
            pending_rx: VecDeque::new(),
            tx_prev_high: false,
            waiting_for_trigger_release: false,
            rx_high: true,
            echo_ns_remaining: None,
        }
    }
}

impl UltrasonicDevice {
    pub(crate) fn push_response(&mut self, tx: u8) {
        if tx == 0x55 {
            let distance_mm = (self.distance_cm.max(0.0) * 10.0).round() as u16;
            self.pending_rx.push_back((distance_mm >> 8) as u8);
            self.pending_rx.push_back((distance_mm & 0xFF) as u8);
        } else if tx == 0x50 {
            self.pending_rx.push_back(25_u8);
        }
    }

    pub(crate) fn pop_response(&mut self) -> Option<u8> {
        self.pending_rx.pop_front()
    }

    pub(crate) fn sample_trigger(&mut self, tx_high: bool) {
        if tx_high && !self.tx_prev_high {
            self.waiting_for_trigger_release = true;
            self.rx_high = true;
            self.echo_ns_remaining = None;
        } else if !tx_high && self.tx_prev_high && self.waiting_for_trigger_release {
            self.echo_ns_remaining = Some(self.distance_ns());
        }
        self.tx_prev_high = tx_high;
    }

    pub(crate) fn tick_ns(&mut self, elapsed_ns: u64) {
        let Some(remaining) = self.echo_ns_remaining else {
            return;
        };
        if remaining <= elapsed_ns {
            self.rx_high = false;
            self.echo_ns_remaining = None;
            self.waiting_for_trigger_release = false;
        } else {
            self.echo_ns_remaining = Some(remaining - elapsed_ns);
        }
    }

    pub(crate) fn rx_level(&self) -> bool {
        self.rx_high
    }

    fn distance_ns(&self) -> u64 {
        ((self.distance_cm.max(0.0) / 0.017) * NS_PER_MICROSECOND as f32)
            .round()
            .clamp(0.0, u64::MAX as f32) as u64
    }
}
