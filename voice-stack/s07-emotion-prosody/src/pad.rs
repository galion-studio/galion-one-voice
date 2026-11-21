// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Live mood point from HORBM.
//!
//! The real `horbm::Brain` is not in this crate. [`Pad`] matches the `mood`
//! field of `TurnOut`: arousal, valence, dominance. `Pad::RESTING` is the
//! brain's rest point from the stack contract, `{0.3, 0.25, 0.5}`.
//!
//! Ranges follow `galion-emotion`'s `EmotionFrame` and `PersonaMood`:
//! arousal 0..=1, valence −1..=1, dominance 0..=1. Samantha's resting mood
//! in `galion-emotion/src/persona_mood.rs` is valence 0.25, arousal 0.35,
//! dominance 0.45. This sector recentres the voice on [`Pad::RESTING`] so a
//! brain sitting at rest speaks the Samantha `e0` line.

/// One mood sample. Copy, no heap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    /// HORBM rest. Field order is arousal, valence, dominance.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    /// Finite and inside the emotion-frame ranges. A non-finite component
    /// becomes the resting value, not a poison number.
    pub fn clamped(self) -> Pad {
        Pad {
            arousal: finite_clamp(self.arousal, 0.0, 1.0, Self::RESTING.arousal),
            valence: finite_clamp(self.valence, -1.0, 1.0, Self::RESTING.valence),
            dominance: finite_clamp(self.dominance, 0.0, 1.0, Self::RESTING.dominance),
        }
    }

    pub fn is_rest(self) -> bool {
        let p = self.clamped();
        p.arousal == Self::RESTING.arousal
            && p.valence == Self::RESTING.valence
            && p.dominance == Self::RESTING.dominance
    }
}

impl Default for Pad {
    fn default() -> Self {
        Self::RESTING
    }
}

pub(crate) fn finite_clamp(value: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_matches_the_horbm_contract() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
    }

    #[test]
    fn non_finite_falls_back_to_rest() {
        let p = Pad {
            arousal: f32::NAN,
            valence: f32::INFINITY,
            dominance: f32::NEG_INFINITY,
        }
        .clamped();
        assert_eq!(p, Pad::RESTING);
    }

    #[test]
    fn out_of_range_clamps_to_the_frame() {
        let p = Pad {
            arousal: 4.0,
            valence: -3.0,
            dominance: 2.0,
        }
        .clamped();
        assert_eq!(p.arousal, 1.0);
        assert_eq!(p.valence, -1.0);
        assert_eq!(p.dominance, 1.0);
    }
}
