use std::collections::VecDeque;

use crate::timing::CPU_TICKS_PER_US;

#[derive(Debug)]
pub(crate) struct UltrasonicDevice {
    pub(crate) distance_cm: f32,
    pending_rx: VecDeque<u8>,
    tx_prev_high: bool,
    waiting_for_measure: bool,
    rx_high: bool,
    target_counter: Option<u16>,
}

impl Default for UltrasonicDevice {
    fn default() -> Self {
        Self {
            distance_cm: 0.0,
            pending_rx: VecDeque::new(),
            tx_prev_high: false,
            waiting_for_measure: false,
            rx_high: true,
            target_counter: None,
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
            self.waiting_for_measure = true;
            self.rx_high = true;
            self.target_counter = None;
        } else if !tx_high && self.tx_prev_high && self.waiting_for_measure {
            self.target_counter = Some(self.distance_counter());
        }
        self.tx_prev_high = tx_high;
    }

    pub(crate) fn sample_counter(&mut self, counter: u16, timeout: bool) {
        if timeout {
            self.rx_high = false;
            self.target_counter = None;
            self.waiting_for_measure = false;
            return;
        }

        if let Some(target) = self.target_counter
            && counter >= target
        {
            self.rx_high = false;
            self.target_counter = None;
            self.waiting_for_measure = false;
        }
    }

    pub(crate) fn stop_measurement(&mut self) {
        self.rx_high = true;
        self.target_counter = None;
        self.waiting_for_measure = false;
    }

    pub(crate) fn rx_level(&self) -> bool {
        self.rx_high
    }

    fn distance_counter(&self) -> u16 {
        ((self.distance_cm.max(0.0) / 0.017) * CPU_TICKS_PER_US as f32)
            .round()
            .clamp(0.0, u16::MAX as f32) as u16
    }
}
