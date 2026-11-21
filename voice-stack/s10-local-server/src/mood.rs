// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Mood clock. One step per 80 ms heartbeat. The pad eases toward the last
//! brain target. It does not jump, and it does not invent a sentence.

use crate::reply::Pad;

/// Fraction of the remaining gap closed on each tick.
pub const APPROACH: f32 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct MoodClock {
    pub pad: Pad,
    pub target: Pad,
    pub tick: u64,
}

impl MoodClock {
    pub fn resting() -> Self {
        Self {
            pad: Pad::RESTING,
            target: Pad::RESTING,
            tick: 0,
        }
    }

    pub fn set_target(&mut self, pad: Pad) {
        self.target = pad.sanitised();
    }

    /// Advance one heartbeat. Returns the pad after the step.
    pub fn step(&mut self) -> Pad {
        self.tick = self.tick.saturating_add(1);
        self.pad = self.pad.approach(self.target, APPROACH);
        self.pad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_rest_and_eases() {
        let mut clock = MoodClock::resting();
        assert_eq!(clock.pad, Pad::RESTING);
        assert_eq!(clock.step(), Pad::RESTING);

        clock.set_target(Pad {
            arousal: 1.0,
            valence: 1.0,
            dominance: 1.0,
        });
        let first = clock.step();
        assert!(first.arousal > Pad::RESTING.arousal);
        assert!(first.arousal < 1.0, "one tick must not jump to the target");

        for _ in 0..40 {
            clock.step();
        }
        assert!(clock.pad.arousal > 0.95);
        assert!(clock.pad.valence > 0.95);
        assert!(clock.pad.dominance > 0.95);
        assert_eq!(clock.tick, 42);
    }

    #[test]
    fn non_finite_target_falls_back_to_rest() {
        let mut clock = MoodClock::resting();
        clock.set_target(Pad {
            arousal: f32::NAN,
            valence: f32::INFINITY,
            dominance: -5.0,
        });
        assert_eq!(clock.target.arousal, Pad::RESTING.arousal);
        assert_eq!(clock.target.valence, Pad::RESTING.valence);
        assert_eq!(clock.target.dominance, 0.0);
    }
}
