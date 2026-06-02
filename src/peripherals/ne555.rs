use crate::timing::{CPU_TICKS_PER_US, TICKS_PER_SECOND};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ne555 {
    frequency_hz: f32,
}

impl Ne555 {
    pub(crate) fn set_frequency_hz(&mut self, value: f32) {
        self.frequency_hz = value.max(0.0);
    }
    pub(crate) fn level(&self, ticks: u64) -> bool {
        if self.frequency_hz <= 0.0 {
            return true;
        }
        let effective_hz = self.frequency_hz * (CPU_TICKS_PER_US as f32 / 12.0);
        let period_ticks = (TICKS_PER_SECOND as f32 / effective_hz).max(1.0);
        let half = (period_ticks / 2.0).max(1.0) as u64;
        (ticks / half).is_multiple_of(2)
    }
}
