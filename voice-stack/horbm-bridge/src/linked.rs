// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Adapter from `horbm::Brain` to [`crate::ReplyEngine`].
//!
//! Compiled only with `--features horbm`. The dependency is crate `horbm`
//! at [`crate::HORBM_LANDING_PATH`]. `open` reads the knowledge directory
//! through the read-only card loader and does not create it. Memory and
//! case paths are not opened.
//!
//! `intent` and `arm` are read so a missing field fails here. `tick` is one
//! empathy frame. It does not add a sentence.

use std::path::Path;

use crate::engine::{Reply, ReplyEngine};
use crate::pad::Pad;

pub struct HorbmEngine {
    brain: horbm::Brain,
    pad: Pad,
}

impl HorbmEngine {
    /// Open the brain. The knowledge directory is read when it already exists.
    /// The memory database and the cases directory are not opened.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let _ = (memory_db, cases);
        let brain = crate::cards::brain_from_dir(
            knowledge_dir,
            crate::research::bind_from_env().as_deref(),
        );
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
        let meta = self.brain.tick_frame();
        self.pad = Pad::new(meta.arousal, meta.valence, meta.dominance);
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
    fn adapter_miss_is_exact_and_the_pad_stays_finite() {
        let memory = Path::new("memory-db");
        let knowledge = Path::new("knowledge");
        let cases = Path::new("cases");
        let memory_before = memory.exists();
        let knowledge_before = knowledge.exists();
        let cases_before = cases.exists();
        let mut engine = HorbmEngine::open(memory, knowledge, cases);
        let quiet = engine.turn("hello");
        assert_eq!(quiet.text, "I don't know.");
        assert_eq!(quiet.sources, vec!["honest:unknown".to_string()]);
        assert!(quiet.mood.arousal.is_finite());
        assert!(!quiet.sources.iter().any(|s| s.contains("seam")));
        let held = engine.tick();
        assert!(held.arousal.is_finite());
        assert_eq!(memory.exists(), memory_before);
        assert_eq!(knowledge.exists(), knowledge_before);
        assert_eq!(cases.exists(), cases_before);
    }
}
