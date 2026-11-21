// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! One 80 ms step of the speaking voice.
//!
//! The approach time is the fluid core's `TAU_S` of 0.240 s
//! (`tools/symphony/voice-v2/src/fluid.rs`). Each tick moves the applied
//! [`Pad`] by `1 − exp(−0.080/0.240)` of the remaining distance, then maps
//! that point. A late caller still gets one step: the heartbeat rule is that
//! a late frame is a pause, not a jump.
//!
//! Tags need a sustained mood. One hot tick does not giggle, and a cooldown
//! stops the same hint landing on every line. The reply string is never read.

use crate::map::{giggle_open, inhale_open, map, SpeakingParams, Tag};
use crate::pad::{finite_clamp, Pad};
use crate::TICK_S;

/// Approach time, seconds. Pipeline constant from the voice-v2 fluid core.
pub const TAU_S: f32 = 0.240;

pub const GIGGLE_HOLD_TICKS: u16 = 4;
pub const GIGGLE_COOLDOWN_TICKS: u16 = 50;
pub const INHALE_HOLD_TICKS: u16 = 4;
pub const INHALE_COOLDOWN_TICKS: u16 = 40;

/// `1 − exp(−tick/tau)`. About 0.283 of the gap per 80 ms tick.
pub fn slew_alpha() -> f32 {
    1.0 - (-TICK_S / TAU_S).exp()
}

#[derive(Clone, Copy, Debug)]
struct Gate {
    hold: u16,
    cooldown: u16,
}

impl Gate {
    const CLOSED: Gate = Gate {
        hold: 0,
        cooldown: 0,
    };

    fn poll(&mut self, open: bool, hold_need: u16, cooldown: u16) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            self.hold = 0;
            return false;
        }
        if !open {
            self.hold = 0;
            return false;
        }
        self.hold = self.hold.saturating_add(1);
        if self.hold >= hold_need {
            self.hold = 0;
            self.cooldown = cooldown;
            true
        } else {
            false
        }
    }
}

/// Applied mood, the target the brain last wrote, and the tag gates.
#[derive(Clone, Debug)]
pub struct ProsodyClock {
    applied: Pad,
    target: Pad,
    giggle: Gate,
    inhale: Gate,
    ticks: u64,
}

impl Default for ProsodyClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ProsodyClock {
    pub fn new() -> Self {
        Self {
            applied: Pad::RESTING,
            target: Pad::RESTING,
            giggle: Gate::CLOSED,
            inhale: Gate::CLOSED,
            ticks: 0,
        }
    }

    pub fn applied(&self) -> Pad {
        self.applied
    }

    pub fn target(&self) -> Pad {
        self.target
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Replace the target. A non-finite component keeps the previous target
    /// component. The applied voice does not move until [`Self::tick`].
    pub fn set_target(&mut self, pad: Pad) {
        self.target = Pad {
            arousal: finite_clamp(pad.arousal, 0.0, 1.0, self.target.arousal),
            valence: finite_clamp(pad.valence, -1.0, 1.0, self.target.valence),
            dominance: finite_clamp(pad.dominance, 0.0, 1.0, self.target.dominance),
        };
    }

    /// Take the mood off a turn. `reply` is not read.
    pub fn follow(&mut self, mood: Pad) {
        self.set_target(mood);
    }

    /// One 80 ms step. Stack only.
    pub fn tick(&mut self) -> SpeakingParams {
        let alpha = slew_alpha();
        self.applied = slew_pad(self.applied, self.target, alpha);
        let mut params = map(self.applied);
        let giggle = self.giggle.poll(
            giggle_open(self.applied),
            GIGGLE_HOLD_TICKS,
            GIGGLE_COOLDOWN_TICKS,
        );
        let inhale = self.inhale.poll(
            inhale_open(self.applied, params.pre_pause_ms),
            INHALE_HOLD_TICKS,
            INHALE_COOLDOWN_TICKS,
        );
        params.tag = if giggle {
            Some(Tag::Giggle)
        } else if inhale {
            Some(Tag::Inhale)
        } else {
            None
        };
        self.ticks = self.ticks.saturating_add(1);
        params
    }
}

fn slew_pad(current: Pad, target: Pad, alpha: f32) -> Pad {
    Pad {
        arousal: slew(current.arousal, target.arousal, alpha),
        valence: slew(current.valence, target.valence, alpha),
        dominance: slew(current.dominance, target.dominance, alpha),
    }
}

fn slew(current: f32, target: f32, alpha: f32) -> f32 {
    current + (target - current) * alpha
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{EXAGGERATION_MAX, EXAGGERATION_MIN, EXAGGERATION_REST};

    #[test]
    fn alpha_matches_the_fluid_core() {
        let alpha = slew_alpha();
        let expect = 1.0 - (-0.080f32 / 0.240).exp();
        assert!((alpha - expect).abs() < 1e-6);
        assert!(alpha > 0.28 && alpha < 0.29);
    }

    #[test]
    fn one_tick_does_not_jump_to_the_target() {
        let mut clock = ProsodyClock::new();
        let target = Pad {
            arousal: 1.0,
            valence: 1.0,
            dominance: 1.0,
        };
        clock.set_target(target);
        let params = clock.tick();
        let alpha = slew_alpha();
        let applied = clock.applied();
        assert!(
            (applied.arousal - (Pad::RESTING.arousal + alpha * (1.0 - Pad::RESTING.arousal))).abs()
                < 1e-5
        );
        assert!((params.exaggeration - EXAGGERATION_REST).abs() < 0.05);
        assert!(params.exaggeration < EXAGGERATION_MAX);
        assert!((params.exaggeration - EXAGGERATION_MAX).abs() > 0.02);
        let instant = map(target);
        assert!(params.exaggeration < instant.exaggeration);
    }

    #[test]
    fn many_ticks_land_on_the_instant_map() {
        let target = Pad {
            arousal: 0.82,
            valence: 0.66,
            dominance: 0.42,
        };
        let mut clock = ProsodyClock::new();
        clock.set_target(target);
        let mut last = clock.tick();
        for _ in 0..48 {
            last = clock.tick();
        }
        let instant = map(target);
        assert!((last.exaggeration - instant.exaggeration).abs() < 1e-4);
        assert!((last.length_scale - instant.length_scale).abs() < 1e-4);
        assert!((last.pre_pause_ms - instant.pre_pause_ms).abs() < 1e-3);
        assert!((last.energy - instant.energy).abs() < 1e-4);
        assert!(last.in_bounds());
    }

    #[test]
    fn a_nan_target_does_not_move_rest() {
        let mut clock = ProsodyClock::new();
        let first = clock.tick();
        clock.set_target(Pad {
            arousal: f32::NAN,
            valence: f32::INFINITY,
            dominance: f32::NEG_INFINITY,
        });
        let second = clock.tick();
        assert_eq!(clock.target(), Pad::RESTING);
        assert_eq!(first.exaggeration, second.exaggeration);
        assert_eq!(first.length_scale, second.length_scale);
        assert_eq!(first.energy, second.energy);
        assert!(second.tag.is_none());
    }

    #[test]
    fn rest_ticks_stay_put_and_silent() {
        let mut clock = ProsodyClock::new();
        for _ in 0..30 {
            let p = clock.tick();
            assert_eq!(p.exaggeration, EXAGGERATION_REST);
            assert!(p.tag.is_none());
            assert!((EXAGGERATION_MIN..=EXAGGERATION_MAX).contains(&p.exaggeration));
        }
    }
}
