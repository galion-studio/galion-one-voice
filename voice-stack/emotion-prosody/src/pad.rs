// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Live mood point from HORBM.
//!
//! The real `horbm::Brain` is not in this crate. [`Pad`] matches the `mood`
//! field of `TurnOut`: arousal, valence, dominance. [`Pad::RESTING`] is the
//! owner's triple `{ arousal: 0.3, valence: 0.25, dominance: 0.5 }`.
//!
//! `horbm` does not document a range for `Pad` in this repository.
//! [`ASSUMED_AROUSAL`], [`ASSUMED_VALENCE`], and [`ASSUMED_DOMINANCE`] are
//! this crate's clamp until that range exists. Valence uses −1..=1 because
//! that is the axis `EmotionFrame` documents, not because `horbm` said so.
//! A sample below 0 is negative on this assumption. It is not "a value the
//! brain defined".
//!
//! `EmotionFrame::neutral` shares arousal 0.3 and dominance 0.5 and uses
//! valence 0. Samantha's persona rest in `persona_mood.rs` is a third triple
//! (valence 0.25, arousal 0.35, dominance 0.45). This sector does not copy a
//! `Pad` into an `EmotionFrame`. The voice is recentred on [`Pad::RESTING`]
//! so a brain at rest speaks the Samantha `e0` line.

/// One mood sample. Copy, no heap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

/// This crate's clamp until `horbm` documents a range. Not a published `Pad` bound.
pub const ASSUMED_AROUSAL: (f32, f32) = (0.0, 1.0);
/// Assumed valence axis. Same numbers as `EmotionFrame`, which is a different type.
pub const ASSUMED_VALENCE: (f32, f32) = (-1.0, 1.0);
/// This crate's clamp until `horbm` documents a range. Not a published `Pad` bound.
pub const ASSUMED_DOMINANCE: (f32, f32) = (0.0, 1.0);

impl Pad {
    /// Owner's rest. Field order is arousal, valence, dominance.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    /// Finite and inside [`ASSUMED_AROUSAL`], [`ASSUMED_VALENCE`], and
    /// [`ASSUMED_DOMINANCE`]. A non-finite component becomes the resting
    /// value, not a poison number.
    pub fn clamped(self) -> Pad {
        Pad {
            arousal: finite_clamp(
                self.arousal,
                ASSUMED_AROUSAL.0,
                ASSUMED_AROUSAL.1,
                Self::RESTING.arousal,
            ),
            valence: finite_clamp(
                self.valence,
                ASSUMED_VALENCE.0,
                ASSUMED_VALENCE.1,
                Self::RESTING.valence,
            ),
            dominance: finite_clamp(
                self.dominance,
                ASSUMED_DOMINANCE.0,
                ASSUMED_DOMINANCE.1,
                Self::RESTING.dominance,
            ),
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
    fn out_of_range_uses_this_crates_assumed_clamp() {
        let p = Pad {
            arousal: 4.0,
            valence: -3.0,
            dominance: 2.0,
        }
        .clamped();
        assert_eq!(p.arousal, ASSUMED_AROUSAL.1);
        assert_eq!(p.valence, ASSUMED_VALENCE.0);
        assert_eq!(p.dominance, ASSUMED_DOMINANCE.1);
        // A negative valence is kept when it sits inside the assumption.
        // That does not mean horbm defined negatives.
        let mild = Pad {
            arousal: 0.3,
            valence: -0.4,
            dominance: 0.5,
        }
        .clamped();
        assert_eq!(mild.valence, -0.4);
    }

    #[test]
    fn resting_is_not_an_emotion_frame_neutral() {
        let frame = galion_emotion::EmotionFrame::neutral(0, 0, 80);
        assert_eq!(Pad::RESTING.arousal, frame.arousal);
        assert_eq!(Pad::RESTING.dominance, frame.dominance);
        assert_ne!(Pad::RESTING.valence, frame.valence);
        assert_eq!(frame.valence, 0.0);
        assert_eq!(Pad::RESTING.valence, 0.25);
    }
}
