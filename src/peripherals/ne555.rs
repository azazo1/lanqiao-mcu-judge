use crate::chip::NS_PER_SECOND;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ne555 {
    frequency_hz: f32,
}

impl Ne555 {
    pub(crate) fn set_frequency_hz(&mut self, value: f32) {
        self.frequency_hz = value.max(0.0);
    }

    pub(crate) fn level(&self, time_ns: u64) -> bool {
        let Some(half_period_ns) = self.half_period_ns() else {
            return true;
        };
        transition_count_until(time_ns, half_period_ns).is_multiple_of(2)
    }

    pub(crate) fn falling_edges_between(&self, start_time_ns: u64, end_time_ns: u64) -> u32 {
        let Some(half_period_ns) = self.half_period_ns() else {
            return 0;
        };
        if end_time_ns <= start_time_ns {
            return 0;
        }

        let end_count = falling_edge_count_until(end_time_ns, half_period_ns);
        let start_count = falling_edge_count_until(start_time_ns, half_period_ns);
        end_count.saturating_sub(start_count).min(u32::MAX as u64) as u32
    }

    fn half_period_ns(&self) -> Option<f64> {
        if self.frequency_hz <= 0.0 {
            return None;
        }

        Some((NS_PER_SECOND as f64 / (f64::from(self.frequency_hz) * 2.0)).max(1.0))
    }
}

fn transition_count_until(time_ns: u64, half_period_ns: f64) -> u64 {
    ((time_ns as f64) / half_period_ns).floor().max(0.0) as u64
}

fn falling_edge_count_until(time_ns: u64, half_period_ns: f64) -> u64 {
    transition_count_until(time_ns, half_period_ns).div_ceil(2)
}

#[cfg(test)]
mod tests {
    use super::Ne555;

    #[test]
    fn falling_edges_match_waveform_boundaries() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz(2_200.0);

        let first_half = 227_273_u64;
        assert_eq!(ne555.falling_edges_between(0, first_half - 1), 0);
        assert_eq!(ne555.falling_edges_between(0, first_half), 1);
        assert_eq!(ne555.falling_edges_between(first_half, first_half * 3), 1);
        assert_eq!(ne555.falling_edges_between(0, first_half * 5), 3);
    }
}
