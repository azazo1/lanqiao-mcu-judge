use crate::chip::{CPU_TICKS_PER_US, TICKS_PER_SECOND};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ne555 {
    frequency_hz: f32,
}

impl Ne555 {
    pub(crate) fn set_frequency_hz(&mut self, value: f32) {
        self.frequency_hz = value.max(0.0);
    }

    pub(crate) fn level(&self, ticks: u64) -> bool {
        let Some(half_period_ticks) = self.half_period_ticks() else {
            return true;
        };
        (ticks / half_period_ticks).is_multiple_of(2)
    }

    pub(crate) fn falling_edges_between(&self, start_ticks: u64, end_ticks: u64) -> u32 {
        let Some(half_period_ticks) = self.half_period_ticks() else {
            return 0;
        };
        if end_ticks <= start_ticks {
            return 0;
        }

        let end_count = falling_edge_count_until(end_ticks, half_period_ticks);
        let start_count = falling_edge_count_until(start_ticks, half_period_ticks);
        end_count.saturating_sub(start_count).min(u32::MAX as u64) as u32
    }

    fn half_period_ticks(&self) -> Option<u64> {
        if self.frequency_hz <= 0.0 {
            return None;
        }

        let effective_hz = self.frequency_hz * (CPU_TICKS_PER_US as f32 / 12.0);
        let period_ticks = (TICKS_PER_SECOND as f32 / effective_hz).max(1.0);
        Some((period_ticks / 2.0).max(1.0) as u64)
    }
}

fn falling_edge_count_until(ticks: u64, half_period_ticks: u64) -> u64 {
    if ticks < half_period_ticks {
        return 0;
    }

    (ticks / half_period_ticks).div_ceil(2)
}

#[cfg(test)]
mod tests {
    use super::Ne555;

    #[test]
    fn falling_edges_match_waveform_boundaries() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz(2_200.0);

        let first_half = 2_727_u64;
        assert_eq!(ne555.falling_edges_between(0, first_half - 1), 0);
        assert_eq!(ne555.falling_edges_between(0, first_half), 1);
        assert_eq!(ne555.falling_edges_between(first_half, first_half * 3), 1);
        assert_eq!(ne555.falling_edges_between(0, first_half * 5), 3);
    }
}
