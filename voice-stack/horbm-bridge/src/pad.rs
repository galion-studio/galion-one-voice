// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pleasure-arousal-dominance mood for one Samantha session.
//!
//! [`Pad::RESTING`] is the HORBM rest state from the sector brief:
//! arousal `0.3`, valence `0.25`, dominance `0.5`.
//!
//! [`FRAME_MS`] is `galion-heartbeat`'s `FRAME_MS` (`LOCKS.toml` `[clock]
//! frame_ms = 80`). The brief gives `horbm` no `tick` method, so this
//! wrapper's tick is empty. It does not synthesize a mood trajectory.
//! The next `turn` is what replaces the pad.

/// Heartbeat period. Locked to `galion-heartbeat::FRAME_MS`.
pub const FRAME_MS: u32 = 80;

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
    }
}
