// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Adapter from `horbm::Brain` to [`crate::ReplyEngine`].
//!
//! Compiled only with `--features horbm`. The path dependency is whatever
//! `Cargo.toml` names as `horbm`. Today that is the type seam. When the real
//! crate is at [`crate::HORBM_LANDING_PATH`], point the dependency there.
//! This file should not need an edit if `Brain::open` takes three `&Path`s
//! and returns `Brain`, and `turn` returns the documented `TurnOut`.
//!
//! `intent` and `arm` are read so a missing field fails here. Their types
//! stay on the `horbm` crate. Between turns, [`HorbmEngine::tick`] decays the
//! last Pad toward [`crate::Pad::RESTING`]. The next `turn` replaces that
//! with the brain's Pad.

use std::path::Path;

use crate::engine::{Reply, ReplyEngine};
use crate::pad::Pad;

pub struct HorbmEngine {
    brain: horbm::Brain,
    pad: Pad,
}

impl HorbmEngine {
    /// Open the brain. Paths are the memory database, the knowledge directory,
    /// and the cases directory from `Brain::open`.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let brain = horbm::Brain::open(memory_db, knowledge_dir, cases);
        Self {
            brain,
            pad: Pad::RESTING,
        }
    }
}

impl ReplyEngine for HorbmEngine {
    fn turn(&mut self, text: &str) -> Reply {
        let out = self.brain.turn(text);
        read_field(&out.intent);
        read_field(&out.arm);
        let mood = Pad::new(out.mood.arousal, out.mood.valence, out.mood.dominance);
        self.pad = mood;
        Reply {
            text: out.reply,
            mood,
            sources: out.sources,
            evidence: out.evidence,
            propensity: out.propensity,
            violations: out.violations,
            lookup_us: out.lookup_us,
            took_us: out.took_us,
        }
    }

    fn tick(&mut self) -> Pad {
        self.pad = self.pad.advance_frame();
        self.pad
    }

    fn mood(&self) -> Pad {
        self.pad
    }
}

fn read_field<T>(value: &T) {
    let _ = value as *const T;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReplyEngine;

    #[test]
    fn resting_pad_matches_the_linked_crate() {
        assert_eq!(horbm::Pad::RESTING.arousal, Pad::RESTING.arousal);
        assert_eq!(horbm::Pad::RESTING.valence, Pad::RESTING.valence);
        assert_eq!(horbm::Pad::RESTING.dominance, Pad::RESTING.dominance);
    }

    #[test]
    fn adapter_copies_a_turn_and_then_ticks_toward_rest() {
        let mut engine = HorbmEngine::open(
            Path::new("memory-db"),
            Path::new("knowledge"),
            Path::new("cases"),
        );
        let quiet = engine.turn("hello");
        assert!(quiet.mood.arousal.is_finite());
        if quiet.sources.iter().any(|s| s == "seam:horbm-not-landed") {
            assert!(quiet.text.is_empty());
            let probe = engine.turn("::s04-probe::");
            assert_eq!(probe.text, "Probe reply. Second sentence.");
            assert_eq!(probe.sources, vec!["src:probe".to_string()]);
            assert_eq!(probe.evidence, vec!["card:probe".to_string()]);
            assert!((probe.propensity - 0.42).abs() < 1e-6);
            assert_eq!(probe.violations, 2);
            assert_eq!(probe.lookup_us, 11);
            assert_eq!(probe.took_us, 22);
            assert_eq!(probe.mood, Pad::new(0.7, 0.4, 0.6));
            let decayed = engine.tick();
            assert!(decayed.arousal < 0.7);
            assert!(decayed.arousal > Pad::RESTING.arousal);
        }
    }
}
