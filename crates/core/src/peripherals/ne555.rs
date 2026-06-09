use crate::chip::NS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalEdge {
    Rising,
    Falling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignalTransition {
    pub(crate) time_ns: u64,
    pub(crate) edge: SignalEdge,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SignalTransitionIter {
    phase_anchor_time_ns: u64,
    phase_anchor_position: f64,
    half_period_ns: f64,
    next_transition: u64,
    end_transition: u64,
}

impl SignalTransitionIter {
    pub(crate) fn empty() -> Self {
        Self {
            phase_anchor_time_ns: 0,
            phase_anchor_position: 0.0,
            half_period_ns: 1.0,
            next_transition: 1,
            end_transition: 0,
        }
    }
}

impl Iterator for SignalTransitionIter {
    type Item = SignalTransition;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_transition > self.end_transition {
            return None;
        }

        let transition_count = self.next_transition;
        self.next_transition = self.next_transition.saturating_add(1);
        Some(SignalTransition {
            time_ns: transition_time_ns(
                self.phase_anchor_time_ns,
                self.phase_anchor_position,
                transition_count,
                self.half_period_ns,
            ),
            edge: edge_for_transition(transition_count),
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ne555 {
    frequency_hz: f32,
    phase_anchor_time_ns: u64,
    phase_anchor_position: f64,
}

impl Ne555 {
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

    pub(crate) fn transitions_between(
        &self,
        start_time_ns: u64,
        end_time_ns: u64,
    ) -> SignalTransitionIter {
        if end_time_ns <= start_time_ns {
            return SignalTransitionIter::empty();
        }
        let Some(half_period_ns) = self.half_period_ns() else {
            return SignalTransitionIter::empty();
        };
        let Some(end_position) = self.phase_position_at(end_time_ns) else {
            return SignalTransitionIter::empty();
        };
        let Some(start_position) = self.phase_position_at(start_time_ns) else {
            return SignalTransitionIter::empty();
        };

        let end_transition = transition_count_from_position(end_position);
        let start_transition = transition_count_from_position(start_position);
        if end_transition <= start_transition {
            return SignalTransitionIter::empty();
        }

        SignalTransitionIter {
            phase_anchor_time_ns: self.phase_anchor_time_ns,
            phase_anchor_position: self.phase_anchor_position,
            half_period_ns,
            next_transition: start_transition.saturating_add(1),
            end_transition,
        }
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

fn edge_for_transition(transition_count: u64) -> SignalEdge {
    if transition_count.is_multiple_of(2) {
        SignalEdge::Rising
    } else {
        SignalEdge::Falling
    }
}

fn transition_time_ns(
    phase_anchor_time_ns: u64,
    phase_anchor_position: f64,
    transition_count: u64,
    half_period_ns: f64,
) -> u64 {
    let delta_positions = (transition_count as f64 - phase_anchor_position).max(0.0);
    let elapsed_ns = (delta_positions * half_period_ns - 1e-9).max(0.0).ceil();
    phase_anchor_time_ns.saturating_add(elapsed_ns.min(u64::MAX as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::{Ne555, SignalEdge, SignalTransition};

    fn transitions_between(
        ne555: &Ne555,
        start_time_ns: u64,
        end_time_ns: u64,
    ) -> Vec<SignalTransition> {
        ne555
            .transitions_between(start_time_ns, end_time_ns)
            .collect()
    }

    #[test]
    fn falling_edges_match_waveform_boundaries() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz_at(0, 2_200.0);

        let first_half = 227_273_u64;
        assert_eq!(transitions_between(&ne555, 0, first_half - 1).len(), 0);
        assert_eq!(
            transitions_between(&ne555, 0, first_half)
                .into_iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            1
        );
        assert_eq!(
            transitions_between(&ne555, first_half, first_half * 3)
                .into_iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            1
        );
        assert_eq!(
            transitions_between(&ne555, 0, first_half * 5)
                .into_iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            3
        );
    }

    #[test]
    fn transitions_distinguish_rising_and_falling() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz_at(0, 1_000.0);

        let transitions = transitions_between(&ne555, 0, 2_000_000);
        assert_eq!(
            transitions
                .iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            2
        );
        assert_eq!(
            transitions
                .iter()
                .filter(|transition| transition.edge == SignalEdge::Rising)
                .count(),
            2
        );
    }

    #[test]
    fn frequency_change_keeps_phase_progress() {
        let mut ne555 = Ne555::default();
        ne555.set_frequency_hz_at(0, 1_000.0);

        let change_time_ns = 400_000_u64;
        assert!(ne555.level(change_time_ns));

        ne555.set_frequency_hz_at(change_time_ns, 2_000.0);

        assert_eq!(
            transitions_between(&ne555, change_time_ns, 449_999)
                .into_iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            0
        );
        assert_eq!(
            transitions_between(&ne555, change_time_ns, 450_001)
                .into_iter()
                .filter(|transition| transition.edge == SignalEdge::Falling)
                .count(),
            1
        );
        assert!(ne555.level(449_999));
        assert!(!ne555.level(450_001));
    }
}
