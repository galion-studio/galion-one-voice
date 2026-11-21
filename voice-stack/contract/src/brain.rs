// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM adapter surface. The body of [`Brain::turn`] is crate `horbm`.
//! This module does not retrieve cards and does not advance a second mood.

use std::path::Path;

/// Brain mood. Field order is the owner's: arousal, valence, dominance.
///
/// [`Pad::RESTING`] is `{ arousal: 0.3, valence: 0.25, dominance: 0.5 }`.
/// `EmotionFrame::neutral` uses the same arousal and dominance and valence 0,
/// on a valence axis documented as −1..+1. This crate does not define a range
/// for `Pad`. Prosody reads `Pad` through the mapper in S07 and does not
/// copy the fields into an `EmotionFrame`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Self = Self {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    pub fn is_finite(self) -> bool {
        self.arousal.is_finite() && self.valence.is_finite() && self.dominance.is_finite()
    }
}

/// One brain turn. `Intent` and `Arm` are the types crate `horbm` uses.
/// The unit defaults exist so this crate compiles without that dependency.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnOut<Intent = (), Arm = ()> {
    pub reply: String,
    pub intent: Intent,
    pub arm: Arm,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
}

/// Paths passed to [`Brain::open`]. The strings are this stack's defaults,
/// not a format claim about the database file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainPaths {
    pub memory_db: std::path::PathBuf,
    pub knowledge_dir: std::path::PathBuf,
    pub cases: std::path::PathBuf,
}

impl BrainPaths {
    /// `$GALION_HOME` when set, otherwise `G:\galion` on Windows and
    /// `$XDG_DATA_HOME/galion` or `~/.local/share/galion` on other hosts.
    pub fn under_home(home: &Path) -> Self {
        Self {
            memory_db: home.join("memory").join("brain.db"),
            knowledge_dir: home.join("knowledge"),
            cases: home.join("cases"),
        }
    }
}

/// What S04 implements by wrapping `horbm::Brain`.
///
/// `tick` runs once per 80 ms frame on the bus thread and does not speak.
/// If the upstream crate advances mood only inside `turn`, the wrapper's
/// `tick` is empty and the PR says so. The wrapper does not invent a mood.
///
/// A speculative transcript (VAD `Speculate`, before `Commit`) must not call
/// a `turn` that writes memory. Consent `remember` defaults to deny. Writes
/// happen after a committed reply, and only when that bit is on.
pub trait Brain {
    type Intent;
    type Arm;
    type Error;

    fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn turn(&mut self, heard: &str) -> Result<TurnOut<Self::Intent, Self::Arm>, Self::Error>;

    fn tick(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmotionFrame;

    #[test]
    fn resting_pad_is_the_owner_triple() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
        assert!(Pad::RESTING.is_finite());
    }

    #[test]
    fn resting_pad_is_not_an_emotion_frame_neutral() {
        let n = EmotionFrame::neutral(0, 0, 80);
        // Arousal 0.3 and dominance 0.5 match EmotionFrame::neutral.
        // Valence does not: 0.25 here, 0.0 on the frame, and that frame's
        // valence runs −1..+1. A field copy is still the wrong conversion.
        assert_eq!(Pad::RESTING.arousal, n.arousal);
        assert_eq!(Pad::RESTING.dominance, n.dominance);
        assert_ne!(Pad::RESTING.valence, n.valence);
        assert_eq!(n.valence, 0.0);
    }
}
