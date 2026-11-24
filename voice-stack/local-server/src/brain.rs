// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local HORBM mind. A turn is [`galion_sam_horbm::CardEngine`], which calls
//! `horbm::Brain::turn`. Empathy sets the mood, cards answer first, a deep
//! miss may ask the research gateway, and any other miss is `I don't know.`
//! Memory and case files are not opened.
//!
//! `Brain::open` in `voice-stack/pipeline-orchestrator` still returns
//! `horbm is not linked`. That is not this process. The card index crate is
//! `horbm-memory` in `galion-knowledge` `v2/memory` (revision `761a7f22`).
//! This process does not fetch that crate and does not create its sqlite
//! file. Cards stay read-only. There is no knowledge-ro listener on
//! 127.0.0.1:7900 in this repository.

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

    /// `GALION_HORBM_KNOWLEDGE` when that directory already exists. On the
    /// laptop, `G:\galion\horbm\knowledge` is next. Then `var/knowledge` when
    /// it already holds a card. Otherwise the tracked example card. Nothing
    /// is created.
    pub fn from_env() -> Self {
        Self::open(&knowledge_dir())
    }

    /// Cards the open directory contributed. One for the shipped kettle note.
    pub fn knowledge_cards(&self) -> usize {
        self.engine.cards()
    }

    pub fn knowledge_path(&self) -> &Path {
        self.engine.knowledge_dir()
    }
}

pub fn knowledge_dir() -> PathBuf {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge");
    let var_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../var/knowledge");
    let windows = PathBuf::from(galion_sam_horbm::WINDOWS_KNOWLEDGE_DIR);
    select_knowledge_dir(
        std::env::var("GALION_HORBM_KNOWLEDGE").ok().as_deref(),
        &windows,
        &var_dir,
        &example,
    )
}

/// First directory that already exists, in the order the 7881 process uses.
pub fn select_knowledge_dir(
    env_value: Option<&str>,
    windows: &Path,
    var_dir: &Path,
    example: &Path,
) -> PathBuf {
    if let Some(dir) = galion_sam_horbm::knowledge_dir_from_env(env_value) {
        return dir;
    }
    if windows.is_dir() {
        return windows.to_path_buf();
    }
    if has_card(var_dir) {
        return var_dir.to_path_buf();
    }
    example.to_path_buf()
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
        let meta = self.engine.mood_meta();
        Ok(TurnOut {
            reply: out.text,
            intent_json: format!("\"{}\"", meta.intent),
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
            empathy_e: meta.e,
            prosody_rate: meta.rate,
            prosody_energy: meta.energy,
            prosody_pitch_st: meta.pitch_st,
            prosody_pause_ms: meta.pause_ms,
        })
    }

    fn tick(&mut self) -> Option<Pad> {
        let pad = self.engine.tick();
        Some(Pad {
            arousal: pad.arousal,
            valence: pad.valence,
            dominance: pad.dominance,
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
        let tired = mind.turn("I am so tired").unwrap();
        assert_eq!(tired.reply, "I don't know.");
        assert!(tired.empathy_e > 0.0);
        assert!(tired.mood.valence > 0.0);
        assert!(tired.prosody_rate.is_finite());
        assert!(!tired.reply.to_ascii_lowercase().contains("sorry"));
        assert_eq!(names(&dir), names_before);
        assert!(!dir.join("memory").exists());
        assert_eq!(mind.knowledge_cards(), 1);
        assert_eq!(mind.knowledge_path(), dir);
    }

    #[test]
    fn knowledge_dir_falls_back_to_the_example_card() {
        let root = std::env::temp_dir().join(format!(
            "horbm-knowledge-pick-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let example = root.join("example");
        let windows = root.join("windows");
        let var_dir = root.join("var");
        std::fs::create_dir_all(&example).unwrap();
        let picked = select_knowledge_dir(None, &windows, &var_dir, &example);
        assert_eq!(picked, example);
        std::fs::create_dir_all(&windows).unwrap();
        let picked = select_knowledge_dir(
            Some("/no/such/horbm/knowledge"),
            &windows,
            &var_dir,
            &example,
        );
        assert_eq!(picked, windows);
        let env_dir = root.join("env");
        std::fs::create_dir_all(&env_dir).unwrap();
        let picked = select_knowledge_dir(
            Some(env_dir.to_str().unwrap()),
            &windows,
            &var_dir,
            &example,
        );
        assert_eq!(picked, env_dir);
        let _ = std::fs::remove_dir_all(&root);
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
