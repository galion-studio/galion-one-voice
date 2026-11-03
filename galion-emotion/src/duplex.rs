//! One feeling decision per heartbeat frame.
//!
//! [`EmotionClock::on_frame`] is the duplex host's call. It runs after
//! `Heartbeat::step` returns. It is not inside that step. The decision is
//! [`crate::StubExpressor::voice_color`]: weight `e`, prosody, and an OKLCH
//! colour. There is no reply line on this clock.
//!
//! Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

use crate::{EmotionFrame, StubExpressor, VoiceColor};

/// Host-side feeling clock. One [`VoiceColor`] per 80 ms frame.
///
/// Stack only. Call it once per frame, on the host, after the heartbeat
/// step. A late frame is still the host's pause; this clock does not catch up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmotionClock {
    last: VoiceColor,
    frames: u32,
}

impl Default for EmotionClock {
    fn default() -> Self {
        Self::new()
    }
}

impl EmotionClock {
    pub fn new() -> Self {
        Self {
            last: StubExpressor.voice_color(None),
            frames: 0,
        }
    }

    /// Frames accepted so far. The host multiplies by 80 ms.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn last(&self) -> VoiceColor {
        self.last
    }

    /// Shared outbound decision for this frame. `None` is no consent, so `e` is 0.
    pub fn on_frame(&mut self, caller: Option<&EmotionFrame>) -> VoiceColor {
        let color = StubExpressor.voice_color(caller);
        self.last = color;
        self.frames = self.frames.wrapping_add(1);
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Affect, EMPATHY_OKLCH, NEUTRAL_OKLCH};

    fn frame(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
        EmotionFrame {
            seq: 1,
            t_ms: 80,
            dur_ms: 80,
            valence: v,
            arousal: a,
            dominance: d,
            confidence: c,
        }
    }

    #[test]
    fn each_call_is_one_frame_and_a_tired_caller_reaches_heather() {
        let mut clock = EmotionClock::new();
        assert_eq!(clock.last().e, 0.0);
        let tired = frame(-0.6, 0.2, 0.4, 1.0);
        assert_eq!(tired.label(), Affect::Tired);
        let color = clock.on_frame(Some(&tired));
        assert_eq!(clock.frames(), 1);
        assert!((color.e - 1.0).abs() < 1e-6);
        assert_eq!(color.oklch, EMPATHY_OKLCH);
        assert!(color.prosody.pitch_shift_st < 0.0);
        assert_eq!(color.prosody.pre_pause_ms, 250);
    }

    #[test]
    fn joy_and_a_missing_frame_stay_at_the_neutral_mark() {
        let mut clock = EmotionClock::new();
        let joy = clock.on_frame(Some(&frame(0.8, 0.8, 0.5, 0.9)));
        assert_eq!(joy.e, 0.0);
        assert_eq!(joy.oklch, NEUTRAL_OKLCH);
        let quiet = clock.on_frame(None);
        assert_eq!(quiet.e, 0.0);
        assert_eq!(clock.frames(), 2);
    }

    #[test]
    fn the_heartbeat_step_does_not_own_this_clock() {
        let src = include_str!("../../crates/galion-heartbeat/src/lib.rs");
        assert!(!src.contains("EmotionClock"));
        assert!(!src.contains("voice_color"));
        assert!(!src.contains("hub_texture"));
        assert!(src.contains("never allocates"));
        assert!(src.contains("late"));
    }
}
