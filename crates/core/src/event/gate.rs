use std::{
    array,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use crate::{event::track::EventTrack, wave::WaveCaptureWindow};

#[derive(Debug)]
pub(crate) struct EventGate {
    wave_window: WaveCaptureWindow,
    script_refcounts: [AtomicU32; EventTrack::COUNT],
}

pub(crate) type SharedEventGate = Arc<EventGate>;

#[derive(Debug)]
pub(crate) struct ScriptTrackGuard {
    gate: SharedEventGate,
    track: EventTrack,
}

impl Drop for ScriptTrackGuard {
    fn drop(&mut self) {
        self.gate.disable_script_track(self.track);
    }
}

impl EventGate {
    pub(crate) fn shared(wave_window: WaveCaptureWindow) -> SharedEventGate {
        Arc::new(Self {
            wave_window,
            script_refcounts: array::from_fn(|_| AtomicU32::new(0)),
        })
    }

    pub(crate) fn need_wave_event(&self, time_ns: u64) -> bool {
        self.wave_window.includes(time_ns)
    }

    pub(crate) fn need_script_track(&self, track: EventTrack) -> bool {
        self.script_refcounts[track.index()].load(Ordering::Relaxed) > 0
    }

    pub(crate) fn need_direct_event(&self, track: EventTrack, time_ns: u64) -> bool {
        self.need_script_track(track) || self.need_wave_event(time_ns)
    }

    pub(crate) fn enable_script_track(
        self: &SharedEventGate,
        track: EventTrack,
    ) -> ScriptTrackGuard {
        self.script_refcounts[track.index()].fetch_add(1, Ordering::Relaxed);
        ScriptTrackGuard {
            gate: Arc::clone(self),
            track,
        }
    }

    fn disable_script_track(&self, track: EventTrack) {
        let count = self.script_refcounts[track.index()].load(Ordering::Relaxed);
        debug_assert!(count > 0, "script track refcount underflow: {track:?}");
        if count > 0 {
            self.script_refcounts[track.index()].fetch_sub(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{event::track::EventTrack, wave::WaveCaptureWindow};

    use super::EventGate;

    #[test]
    fn gate_tracks_script_refcounts_and_wave_window() {
        let gate = EventGate::shared(WaveCaptureWindow::bounded(100, Some(200)));
        assert!(!gate.need_direct_event(EventTrack::Uart1, 50));
        assert!(gate.need_direct_event(EventTrack::Uart1, 150));

        {
            let _guard0 = gate.enable_script_track(EventTrack::Uart1);
            let _guard1 = gate.enable_script_track(EventTrack::Uart1);
            assert!(gate.need_direct_event(EventTrack::Uart1, 50));
        }

        assert!(!gate.need_direct_event(EventTrack::Uart1, 50));
    }
}
