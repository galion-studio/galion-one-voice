// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! What the heartbeat host may tell the nerve after a frame, not during it.
//!
//! `Heartbeat::step` forbids channels, locks, and allocation. Call
//! [`note_from_frame`] on the returned `paused` flag afterwards, then
//! [`crate::NerveEmitter::note`] if it returns `Some`. A healthy frame is
//! silence: the nerve does not get one line per tick.

use crate::nerve::VoiceNote;

/// `paused` is `FrameOut::paused` (the scheduler marked the frame late, or the
/// you-band PCM was not one frame long). That pause is `voice.frame_miss`.
///
/// Fallback and backend-up are not inferred here. The backend supervisor
/// emits those with [`VoiceNote::fallback`](crate::VoiceNote::fallback) and
/// [`VoiceNote::backend_up`](crate::VoiceNote::backend_up).
pub fn note_from_frame(paused: bool, frame: u32) -> Option<VoiceNote> {
    if paused {
        Some(VoiceNote::frame_miss(frame))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NerveGroup, Severity, VoiceKind};

    #[test]
    fn healthy_frame_is_quiet() {
        assert!(note_from_frame(false, 0).is_none());
    }

    #[test]
    fn paused_frame_is_a_miss_on_galion_os() {
        let n = note_from_frame(true, 12).unwrap();
        assert_eq!(n.kind, VoiceKind::FrameMiss);
        assert_eq!(n.group, NerveGroup::GalionOs);
        assert_eq!(n.severity, Severity::Medium);
        assert_eq!(n.frame, 12);
        assert_eq!(n.kind.as_str(), "voice.frame_miss");
    }
}
