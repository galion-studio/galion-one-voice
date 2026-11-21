// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pleasure-arousal-dominance mood for one Samantha session.
//!
//! [`Pad::RESTING`] is the HORBM rest state from the sector brief:
//! arousal `0.3`, valence `0.25`, dominance `0.5`.
//!
//! Between turns the mood moves toward that rest on the voice heartbeat.
//! [`FRAME_MS`] is `galion-heartbeat`'s `FRAME_MS` (`LOCKS.toml` `[clock]
//! frame_ms = 80`). The half-life is Samantha's
//! `MoodDynamics::half_life_ms` in `galion-emotion` (`20_000` ms). One tick
//! uses the same exponential step as `PersonaMood::tick`:
//! `rest + (mood - rest) * 0.5^(dt / half_life)`.

/// Heartbeat period. Locked to `galion-heartbeat::FRAME_MS`.
pub const FRAME_MS: u32 = 80;

/// Samantha's decay half-life, milliseconds.
/// `galion-emotion` `SAMANTHA.dynamics.half_life_ms`.
pub const MOOD_HALF_LIFE_MS: f32 = 20_000.0;

/// One mood sample. Named fields, in the brief's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    /// HORBM rest. `{arousal: 0.3, valence: 0.25, dominance: 0.5}`.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    /// Finite components, in the same ranges `galion-emotion` uses for VAD.
    /// A non-finite component falls back to [`Pad::RESTING`] on that axis.
    pub fn new(arousal: f32, valence: f32, dominance: f32) -> Self {
        Self {
            arousal,
            valence,
            dominance,
        }
        .clamped()
    }

    pub fn clamped(self) -> Self {
        let axis = |x: f32, lo: f32, hi: f32, fallback: f32| {
            if x.is_finite() {
                x.clamp(lo, hi)
            } else {
                fallback
            }
        };
        Self {
            arousal: axis(self.arousal, 0.0, 1.0, Self::RESTING.arousal),
            valence: axis(self.valence, -1.0, 1.0, Self::RESTING.valence),
            dominance: axis(self.dominance, 0.0, 1.0, Self::RESTING.dominance),
        }
    }

    /// One 80 ms frame toward [`Pad::RESTING`]. Stack only.
    pub fn advance_frame(self) -> Self {
        self.advance_ms(FRAME_MS)
    }

    /// Decay toward rest over `dt_ms`. `0` leaves the value clamped.
    pub fn advance_ms(self, dt_ms: u32) -> Self {
        if dt_ms == 0 || MOOD_HALF_LIFE_MS <= 0.0 {
            return self.clamped();
        }
        let keep = 0.5f32.powf(dt_ms as f32 / MOOD_HALF_LIFE_MS);
        Self::RESTING.lerp(self, keep).clamped()
    }

    /// `self` at `t == 0`, `other` at `t == 1`.
    fn lerp(self, other: Self, t: f32) -> Self {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            arousal: self.arousal + (other.arousal - self.arousal) * t,
            valence: self.valence + (other.valence - self.valence) * t,
            dominance: self.dominance + (other.dominance - self.dominance) * t,
        }
    }
}

impl Default for Pad {
    fn default() -> Self {
        Self::RESTING
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_is_the_horbm_pad() {
        assert_eq!(
            (
                Pad::RESTING.arousal,
                Pad::RESTING.valence,
                Pad::RESTING.dominance
            ),
            (0.3, 0.25, 0.5)
        );
        assert_eq!(FRAME_MS, 80);
        assert_eq!(MOOD_HALF_LIFE_MS, 20_000.0);
    }

    #[test]
    fn frame_constant_matches_the_heartbeat_and_locks() {
        let heartbeat = include_str!("../../../crates/galion-heartbeat/src/lib.rs");
        assert!(
            heartbeat.contains("pub const FRAME_MS: u32 = 80"),
            "galion-heartbeat FRAME_MS changed"
        );
        let locks = include_str!("../../../LOCKS.toml");
        assert!(locks.contains("frame_ms = 80"));
        let mood = include_str!("../../../galion-emotion/src/persona_mood.rs");
        assert!(
            mood.contains("half_life_ms: 20_000.0"),
            "Samantha half-life changed"
        );
        assert!(mood.contains("fn tick(&mut self, now_ms: u64)"));
    }

    #[test]
    fn one_frame_matches_the_half_life_step() {
        let start = Pad::new(0.8, 0.6, 0.7);
        let got = start.advance_frame();
        let keep = 0.5f32.powf(FRAME_MS as f32 / MOOD_HALF_LIFE_MS);
        let expect = |from: f32, rest: f32| rest + (from - rest) * keep;
        assert!((got.arousal - expect(0.8, 0.3)).abs() < 1e-6);
        assert!((got.valence - expect(0.6, 0.25)).abs() < 1e-6);
        assert!((got.dominance - expect(0.7, 0.5)).abs() < 1e-6);
        assert!(got.arousal < start.arousal);
        assert!(got.arousal > Pad::RESTING.arousal);
    }

    #[test]
    fn rest_stays_at_rest_and_ten_seconds_matches_one_hundred_twenty_five_frames() {
        assert_eq!(Pad::RESTING.advance_frame(), Pad::RESTING);
        let start = Pad::new(0.7, 0.55, 0.5);
        let mut stepped = start;
        for _ in 0..125 {
            stepped = stepped.advance_frame();
        }
        let once = start.advance_ms(10_000);
        assert!((stepped.arousal - once.arousal).abs() < 1e-5);
        assert!((stepped.valence - once.valence).abs() < 1e-5);
        let keep = 0.5f32.powf(0.5);
        let expect = 0.3 + (0.7 - 0.3) * keep;
        assert!((once.arousal - expect).abs() < 1e-5);
    }

    #[test]
    fn non_finite_components_fall_back_per_axis() {
        let raw = Pad {
            arousal: f32::NAN,
            valence: f32::INFINITY,
            dominance: -5.0,
        };
        let got = raw.clamped();
        assert_eq!(got.arousal, Pad::RESTING.arousal);
        assert_eq!(got.valence, Pad::RESTING.valence);
        assert_eq!(got.dominance, 0.0);
        let all_nan = Pad {
            arousal: f32::NAN,
            valence: f32::NAN,
            dominance: f32::NAN,
        };
        assert_eq!(all_nan.advance_frame(), Pad::RESTING);
    }
}
