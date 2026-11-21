// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local HORBM mind. A turn goes through [`galion_sam_horbm::CardEngine`]
//! into `ReasoningRouter::turn`. Cards answer first. Memory and case files
//! are not opened.
//!
//! `Brain::open` in `voice-stack/pipeline-orchestrator` returns
//! `horbm is not linked`. The card index crate is `horbm-memory` in
//! `galion-knowledge` `v2/memory` (revision `761a7f22`). This process does
//! not fetch that crate and does not create its sqlite file. A query that
//! matches no card is `I don't know.`

use std::path::{Path, PathBuf};

use galion_sam_horbm::{CardEngine, ReplyEngine as HorbmReply};

use crate::reply::{Pad, ReplyEngine, TurnOut};

pub struct HorbmMind {
    engine: CardEngine,
}

impl HorbmMind {
    pub fn open(knowledge_dir: &Path) -> Self {
        Self {
            engine: CardEngine::open(Path::new("memory"), knowledge_dir, Path::new("cases")),
        }
    }

    /// `GALION_HORBM_KNOWLEDGE` when it is set. Otherwise `var/knowledge`
    /// under the repo when that directory already holds a card, otherwise
    /// the tracked examples next to this crate. Nothing is created.
    pub fn from_env() -> Self {
        Self::open(&knowledge_dir())
    }
}

pub fn knowledge_dir() -> PathBuf {
    if let Ok(raw) = std::env::var("GALION_HORBM_KNOWLEDGE") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let var_dir = repo.join("var/knowledge");
    if has_card(&var_dir) {
        return var_dir;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")
}

fn has_card(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        matches!(
            entry.path().extension().and_then(|s| s.to_str()),
            Some("md" | "json")
        )
    })
}

impl ReplyEngine for HorbmMind {
    fn name(&self) -> &str {
        "horbm"
    }

    fn turn(&mut self, heard: &str) -> Result<TurnOut, String> {
        let out = self.engine.turn(heard);
        Ok(TurnOut {
            reply: out.text,
            intent_json: "null".into(),
            arm_json: "null".into(),
            propensity: out.propensity,
            evidence: out.evidence,
            sources: out.sources,
            violations: out.violations,
            lookup_us: out.lookup_us,
            took_us: out.took_us,
            mood: Pad {
                arousal: out.mood.arousal,
                valence: out.mood.valence,
                dominance: out.mood.dominance,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_sam_horbm::UNKNOWN_REPLY as BRIDGE_UNKNOWN;

    #[test]
    fn shipped_card_answers_and_a_miss_does_not_guess() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge");
        let names_before = names(&dir);
        let mut mind = HorbmMind::open(&dir);
        let hit = mind.turn("where is the kettle").unwrap();
        assert_eq!(hit.reply, "The kettle is on the stove.");
        assert!(hit.sources.iter().any(|s| s == "card:kettle"));
        assert!(!hit.reply.contains("brain-unlinked"));
        let miss = mind.turn("who won the match").unwrap();
        assert_eq!(miss.reply, "I don't know.");
        assert_eq!(miss.reply, BRIDGE_UNKNOWN);
        assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
        assert_eq!(names(&dir), names_before);
        assert!(!dir.join("memory").exists());
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        names
    }
}
