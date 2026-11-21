// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Compile seam for `samantha-brain`'s `horbm` feature.
//!
//! This is the documented `Brain::open` / `Brain::turn` surface so the adapter
//! type-checks before the real crate is in the tree. It does not open a
//! memory database, does not read knowledge cards, and does not answer.
//! `turn` returns an empty line and the source `seam:horbm-not-landed`.
//!
//! The input `::s04-probe::` is a fixture for the adapter test. It is not a
//! retrieval query.
//!
//! Replace the path dependency in the parent `Cargo.toml` with
//! `../../horbm/crates/horbm` when that crate lands. This directory is not
//! that crate.

use std::path::Path;

/// Pleasure-arousal-dominance mood. Field order matches the sector brief:
/// arousal, valence, dominance. `RESTING` is `{0.3, 0.25, 0.5}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };
}

/// One brain turn. `intent` and `arm` stay on this crate; the adapter only
/// checks that the fields exist.
#[derive(Clone, Debug, PartialEq)]
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

/// Stand-in brain. `open` does not touch the filesystem.
#[derive(Debug, Default)]
pub struct Brain;

impl Brain {
    pub fn open(_memory_db: &Path, _knowledge_dir: &Path, _cases: &Path) -> Self {
        Self
    }

    pub fn turn(&self, text: &str) -> TurnOut {
        if text == "::s04-probe::" {
            return TurnOut {
                reply: "Probe reply. Second sentence.".into(),
                intent: "probe-intent".into(),
                arm: "probe-arm".into(),
                propensity: 0.42,
                evidence: vec!["card:probe".into()],
                sources: vec!["src:probe".into()],
                violations: 2,
                lookup_us: 11,
                took_us: 22,
                mood: Pad {
                    arousal: 0.7,
                    valence: 0.4,
                    dominance: 0.6,
                },
            };
        }
        TurnOut {
            reply: String::new(),
            intent: "seam".into(),
            arm: "seam".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: vec!["seam:horbm-not-landed".into()],
            violations: 0,
            lookup_us: 0,
            took_us: 0,
            mood: Pad::RESTING,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_matches_the_sector_brief() {
        let p = Pad::RESTING;
        assert_eq!((p.arousal, p.valence, p.dominance), (0.3, 0.25, 0.5));
    }

    #[test]
    fn open_ignores_missing_paths_and_does_not_answer() {
        let brain = Brain::open(
            Path::new("memory-db"),
            Path::new("knowledge"),
            Path::new("cases"),
        );
        let out = brain.turn("where is the kettle");
        assert!(out.reply.is_empty());
        assert_eq!(out.sources, vec!["seam:horbm-not-landed".to_string()]);
        assert_eq!(out.mood, Pad::RESTING);
    }
}
