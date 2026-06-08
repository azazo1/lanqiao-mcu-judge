use crate::{event::track::EventTrack, wave::WaveEventNote};

#[derive(Debug, Clone)]
pub(crate) struct EventTracker {
    counts: [u64; EventTrack::COUNT],
    last_notes: [Option<WaveEventNote>; EventTrack::COUNT],
}

impl Default for EventTracker {
    fn default() -> Self {
        Self {
            counts: [0; EventTrack::COUNT],
            last_notes: std::array::from_fn(|_| None),
        }
    }
}

impl EventTracker {
    pub(crate) fn record(&mut self, note: WaveEventNote) {
        let Some(track) = EventTrack::from_track_id(note.track_id) else {
            return;
        };
        let index = track.index();
        self.counts[index] = self.counts[index].saturating_add(1);
        self.last_notes[index] = Some(note);
    }

    pub(crate) fn count(&self, track: EventTrack) -> u64 {
        self.counts[track.index()]
    }

    pub(crate) fn last_note(&self, track: EventTrack) -> Option<&WaveEventNote> {
        self.last_notes[track.index()].as_ref()
    }
}
