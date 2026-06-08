use crate::peripherals::DigitSample;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SegChangeSet {
    changed: bool,
    digit_changed: [bool; 8],
}

impl SegChangeSet {
    pub(crate) fn changed(self) -> bool {
        self.changed
    }

    pub(crate) fn digit_changed(self, digit_index: usize) -> bool {
        self.digit_changed[digit_index]
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SegEventDetector {
    last_digits: Option<[DigitSample; 8]>,
}

impl SegEventDetector {
    pub(crate) fn reset(&mut self) {
        self.last_digits = None;
    }

    pub(crate) fn observe(&mut self, digits: [DigitSample; 8]) -> Option<SegChangeSet> {
        let previous = self.last_digits.replace(digits)?;

        let mut change_set = SegChangeSet::default();
        for digit_index in 0..digits.len() {
            if previous[digit_index] != digits[digit_index] {
                change_set.changed = true;
                change_set.digit_changed[digit_index] = true;
            }
        }

        change_set.changed().then_some(change_set)
    }
}

#[cfg(test)]
mod tests {
    use crate::peripherals::DigitSample;

    use super::SegEventDetector;

    #[test]
    fn seg_detector_ignores_first_snapshot_and_reports_segment_changes() {
        let mut detector = SegEventDetector::default();
        let blank = [DigitSample::default(); 8];
        assert!(detector.observe(blank).is_none());

        let mut changed = blank;
        changed[2] = DigitSample {
            seen: true,
            segments: !0x3F,
        };
        let change_set = detector.observe(changed).expect("change set");
        assert!(change_set.changed());
        assert!(change_set.digit_changed(2));
        assert!(!change_set.digit_changed(1));
    }
}
