// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! PAD mood point handed to HORBM as a mood target.
//!
//! `Pad::RESTING` is the triple the brain publishes: arousal 0.3, valence 0.25,
//! dominance 0.5. This crate clamps valence to −1..1, arousal to 0..1, and
//! dominance to 0..1. That valence span is this crate's assumption. `horbm`
//! does not document a range in this repository. `EmotionFrame::neutral` is
//! valence 0.0 with the same arousal and dominance, so the two valence numbers
//! are not one scale.
//!
//! The real brain advances mood on its own 80 ms tick. [`Pad::step_toward`] is
//! the stand-in used by [`crate::engine::FakeBrain`], not a copy of that brain.

/// One pleasure / arousal / dominance point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    /// HORBM resting mood. Field order is arousal, valence, dominance.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    /// Fraction of the gap closed on one 80 ms tick by [`crate::engine::FakeBrain`].
    /// About a 300 ms half-life: `ln(2) / 0.18 * 80 ms`. The real crate owns its own gain.
    pub const TICK_ALPHA: f32 = 0.18;

    pub fn clamped(self) -> Pad {
        let c = |x: f32, lo: f32, hi: f32, d: f32| {
            if x.is_finite() {
                x.clamp(lo, hi)
            } else {
                d
            }
        };
        Pad {
            arousal: c(self.arousal, 0.0, 1.0, Self::RESTING.arousal),
            valence: c(self.valence, -1.0, 1.0, Self::RESTING.valence),
            dominance: c(self.dominance, 0.0, 1.0, Self::RESTING.dominance),
        }
    }

    /// One heartbeat step from `self` toward `target`. `alpha` is the fraction
    /// of the remaining gap closed this 80 ms. Does not allocate.
    pub fn step_toward(self, target: Pad, alpha: f32) -> Pad {
        let a = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let t = target.clamped();
        let s = self.clamped();
        Pad {
            arousal: s.arousal + (t.arousal - s.arousal) * a,
            valence: s.valence + (t.valence - s.valence) * a,
            dominance: s.dominance + (t.dominance - s.dominance) * a,
        }
        .clamped()
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
    fn resting_matches_the_horbm_literal() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
    }

    #[test]
    fn clamp_replaces_nan_with_rest() {
        let p = Pad {
            arousal: f32::NAN,
            valence: 4.0,
            dominance: -2.0,
        }
        .clamped();
        assert_eq!(p.arousal, Pad::RESTING.arousal);
        assert_eq!(p.valence, 1.0);
        assert_eq!(p.dominance, 0.0);
    }

    #[test]
    fn tick_moves_toward_the_target_and_stays_in_range() {
        let target = Pad {
            arousal: 0.9,
            valence: -0.8,
            dominance: 0.1,
        };
        let mut mood = Pad::RESTING;
        let once = mood.step_toward(target, Pad::TICK_ALPHA);
        assert!(once.arousal > mood.arousal);
        assert!(once.valence < mood.valence);
        assert!(once.dominance < mood.dominance);
        for _ in 0..40 {
            mood = mood.step_toward(target, Pad::TICK_ALPHA);
        }
        assert!((mood.arousal - target.arousal).abs() < 0.01);
        assert!((mood.valence - target.valence).abs() < 0.01);
        assert!((mood.dominance - target.dominance).abs() < 0.01);
        let blow = mood.step_toward(
            Pad {
                arousal: 9.0,
                valence: -9.0,
                dominance: f32::NAN,
            },
            1.0,
        );
        assert!(blow.arousal <= 1.0 && blow.valence >= -1.0);
        assert_eq!(blow.dominance, Pad::RESTING.dominance);
    }
}
