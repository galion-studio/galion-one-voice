// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! What a turn returns. Field names match `horbm::Brain::turn`.
//!
//! The real crate is not linked. It is opened with
//! `Brain::open(memory_db, knowledge_dir, cases)` and depends on
//! `horbm-memory` from `galion-studio/galion-knowledge` revision
//! `761a7f225304fc010379219e5b88c6669dd48f40`. [`FakeBrain::open`] takes
//! those three paths and does not read them.
//!
//! HORBM advances `mood` on its own 80 ms tick. [`FakeBrain::tick`] is one
//! step of that clock for tests. This sector only writes the mood target.

use crate::pad::Pad;
use std::time::Instant;

/// One brain turn. `mood` is the brain's mood after this call, not the target.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
}

/// Drop-in for `horbm::Brain`. Implement this for the real brain later.
pub trait ReplyEngine {
    fn turn(&mut self, text: &str) -> TurnOut;
}

/// Where emotion-in writes the mood target the brain ticks toward.
pub trait MoodTarget {
    fn set_mood_target(&mut self, target: Pad);
    fn mood(&self) -> Pad;
    fn mood_target(&self) -> Pad;
}

/// Stand-in brain. No retrieval, no audio, no knowledge files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FakeBrain {
    mood: Pad,
    target: Pad,
    turns: u32,
}

impl FakeBrain {
    /// Same argument list as `horbm::Brain::open`. The paths are ignored.
    pub fn open(_memory_db: &str, _knowledge_dir: &str, _cases: &str) -> Self {
        Self {
            mood: Pad::RESTING,
            target: Pad::RESTING,
            turns: 0,
        }
    }

    pub fn turns(self) -> u32 {
        self.turns
    }

    /// One 80 ms mood step toward the current target.
    pub fn tick(&mut self) {
        self.mood = self.mood.step_toward(self.target, Pad::TICK_ALPHA);
    }
}

impl Default for FakeBrain {
    fn default() -> Self {
        Self::open("", "", "")
    }
}

impl MoodTarget for FakeBrain {
    fn set_mood_target(&mut self, target: Pad) {
        self.target = target.clamped();
    }
    fn mood(&self) -> Pad {
        self.mood
    }
    fn mood_target(&self) -> Pad {
        self.target
    }
}

impl ReplyEngine for FakeBrain {
    fn turn(&mut self, text: &str) -> TurnOut {
        let started = Instant::now();
        self.turns = self.turns.saturating_add(1);
        self.tick();
        let took_us = crate::report::elapsed_us(started);
        let violations = if text.trim().is_empty() { 1 } else { 0 };
        TurnOut {
            reply: String::from("(fake-brain)"),
            intent: String::from("unscored"),
            arm: String::from("none"),
            propensity: 0.0,
            evidence: vec![format!(
                "mood-target arousal={:.3} valence={:.3} dominance={:.3}",
                self.target.arousal, self.target.valence, self.target.dominance
            )],
            sources: Vec::new(),
            violations,
            lookup_us: 0,
            took_us,
            mood: self.mood,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_ignores_missing_paths_and_rests() {
        let brain = FakeBrain::open("/no/such/memory", "/no/such/knowledge", "/no/such/cases");
        assert_eq!(brain.mood(), Pad::RESTING);
        assert_eq!(brain.mood_target(), Pad::RESTING);
        assert_eq!(brain.turns(), 0);
    }

    #[test]
    fn turn_ticks_mood_toward_the_target() {
        let mut brain = FakeBrain::open("", "", "");
        let target = Pad {
            arousal: 0.9,
            valence: -0.4,
            dominance: 0.2,
        };
        brain.set_mood_target(target);
        let out = brain.turn("hello");
        assert_eq!(out.reply, "(fake-brain)");
        assert_eq!(out.intent, "unscored");
        assert_eq!(out.arm, "none");
        assert_eq!(out.propensity, 0.0);
        assert!(out.sources.is_empty());
        assert_eq!(out.violations, 0);
        assert_eq!(out.lookup_us, 0);
        assert!(out.mood.arousal > Pad::RESTING.arousal);
        assert!(out.mood.valence < Pad::RESTING.valence);
        assert!(out.evidence[0].contains("arousal=0.900"));
        assert_eq!(brain.turns(), 1);
        let empty = brain.turn("  ");
        assert_eq!(empty.violations, 1);
    }
}
