use crate::chip::NS_PER_SECOND;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ne555 {
    frequency_hz: f32,
    phase_anchor_time_ns: u64,
    phase_anchor_position: f64,
}

impl Ne555 {
    pub(crate) fn set_frequency_hz(&mut self, value: f32) {
        self.set_frequency_hz_at(0, value);
    }

    pub(crate) fn set_frequency_hz_at(&mut self, time_ns: u64, value: f32) {
        let new_frequency_hz = value.max(0.0);
        let anchor_position = if self.frequency_hz > 0.0 && new_frequency_hz > 0.0 {
            self.phase_position_at(time_ns)
                .unwrap_or(0.0)
                .rem_euclid(2.0)
        } else {
            0.0
        };
        self.frequency_hz = new_frequency_hz;
        self.phase_anchor_time_ns = time_ns;
        self.phase_anchor_position = anchor_position;
    }

    pub(crate) fn frequency_hz(&self) -> f32 {
        self.frequency_hz
    }

    pub(crate) fn level(&self, time_ns: u64) -> bool {
        let Some(position) = self.phase_position_at(time_ns) else {
            return true;
        };
        transition_count_from_position(position).is_multiple_of(2)
    }

    pub(crate) fn falling_edges_between(&self, start_time_ns: u64, end_time_ns: u64) -> u32 {
        if end_time_ns <= start_time_ns {
            return 0;
        }
        let Some(end_position) = self.phase_position_at(end_time_ns) else {
            return 0;
        };
        let Some(start_position) = self.phase_position_at(start_time_ns) else {
            return 0;
        };

        let end_count = falling_edge_count_from_position(end_position);
        let start_count = falling_edge_count_from_position(start_position);
        end_count.saturating_sub(start_count).min(u32::MAX as u64) as u32
    }

    fn half_period_ns(&self) -> Option<f64> {
        if self.frequency_hz <= 0.0 {
            return None;
        }

        Some((NS_PER_SECOND as f64 / (f64::from(self.frequency_hz) * 2.0)).max(1.0))
    }

    fn phase_position_at(&self, time_ns: u64) -> Option<f64> {
        let half_period_ns = self.half_period_ns()?;
        let elapsed_ns = time_ns.saturating_sub(self.phase_anchor_time_ns) as f64;
        Some(self.phase_anchor_position + elapsed_ns / half_period_ns)
    }
}

fn transition_count_from_position(position: f64) -> u64 {
    position.floor().max(0.0) as u64
}

fn falling_edge_count_from_position(position: f64) -> u64 {
    transition_count_from_position(position).div_ceil(2)
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

    #[test]
    fn frequency_change_keeps_phase_progress() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz(1_000.0);

        let change_time_ns = 400_000_u64;
        assert!(ne555.level(change_time_ns));

        ne555.set_frequency_hz_at(change_time_ns, 2_000.0);

        assert_eq!(ne555.falling_edges_between(change_time_ns, 449_999), 0);
        assert_eq!(ne555.falling_edges_between(change_time_ns, 450_001), 1);
        assert!(ne555.level(449_999));
        assert!(!ne555.level(450_001));
    }
}
