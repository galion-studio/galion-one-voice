// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local HORBM mind. A turn is [`galion_sam_horbm::CardEngine`], which calls
//! `horbm::Brain::turn`. That turn reads the retrieval cell. `Hits` speaks
//! the stored clause. `Miss` is `I don't know.` `Idle` is `remember` denied:
//! cards answer, and the clause fixture covers a card miss. A deep miss on
//! that idle path may ask the research gateway.
//!
//! Remember and learn are the words in [`crate::host::HostConfig`]. This
//! process does not call [`std::env::set_var`]. `grant` opens the person
//! diary at an absolute path. Any other word leaves it closed. `Brain::open`
//! in `voice-stack/pipeline-orchestrator` still returns `horbm is not linked`.
//! That is not this process. There is no knowledge-ro listener in this crate.

use std::path::{Path, PathBuf};

use galion_sam_horbm::{CardEngine, ReplyEngine as HorbmReply};

use crate::host::HostConfig;
use crate::reply::{Pad, ReplyEngine, TurnOut};

/// One idle thought. Fields are the inner turn and the lean. No prose.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IdleInner {
    pub verb: String,
    pub lean: String,
    pub surprise: f32,
    pub learned: bool,
    pub slots: Vec<String>,
}

pub struct HorbmMind {
    engine: CardEngine,
}

impl HorbmMind {
    /// Scripted and unit-test opener. Remember and learn stay deny. The diary
    /// path is absolute and is not created.
    pub fn open(knowledge_dir: &Path) -> Self {
        let root = std::env::temp_dir().join(format!("galion-horbm-mind-{}", std::process::id()));
        Self::open_configured(knowledge_dir, &HostConfig::deny_under(root))
    }

    /// Live opener. `config.diary_path` is absolute. The directory is created
    /// by [`crate::host::HostSession::boot`] only when a grant needs it.
    pub fn open_configured(knowledge_dir: &Path, config: &HostConfig) -> Self {
        let cases = config.data_root.join("cases");
        Self {
            engine: CardEngine::open_with_remember(
                &config.diary_path,
                knowledge_dir,
                &cases,
                Some(config.remember.word.as_str()),
            ),
        }
    }

    /// `GALION_HORBM_KNOWLEDGE` when that directory already exists. On the
    /// laptop, `G:\galion\horbm\knowledge` is next. Then `var/knowledge` when
    /// it already holds a card. Otherwise the tracked example card. Nothing
    /// is created. Grants stay deny; the live process uses [`Self::open_configured`].
    pub fn from_env() -> Self {
        Self::open(&knowledge_dir())
    }

    /// Id loaded by the brain. This host does not pass a persona name.
    pub fn persona_id(&self) -> String {
        self.engine.brain().persona().id().to_string()
    }

    pub fn person_id(&self) -> String {
        self.engine.brain().person_id().to_string()
    }

    /// [`horbm::PersonMemory::end_session`]. Refused when remember is denied.
    pub fn end_diary_session(&mut self, at: &str) -> horbm::WriteOutcome {
        let person = self.person_id();
        match self.engine.brain_mut().person_memory() {
            Some(memory) => memory.end_session(&person, at),
            None => horbm::WriteOutcome::Refused,
        }
    }

    /// One idle thought. Not called from a turn.
    ///
    /// follow-up: switch to S77's `persona_inner()` / `persona_nudge()` once they are on main.
    pub(crate) fn idle_inner(&mut self) -> Result<IdleInner, String> {
        // Token life does not step the moment. The slower goals do, once.
        let turn = self.engine.brain_mut().inner_turn(0, "", None);
        let note = self
            .engine
            .brain_mut()
            .persona_nudge("", None, false)
            .map_err(|_| "persona-nudge".to_string())?;
        let slots = slot_tokens(self.engine.brain().inner());
        Ok(IdleInner {
            verb: turn.outbound.verb_str().to_string(),
            lean: note.lean().verb.as_str().to_string(),
            surprise: turn.surprise,
            learned: turn.learned,
            slots,
        })
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

fn slot_tokens(inner: &horbm::InnerLife) -> Vec<String> {
    let mut out = Vec::new();
    for index in 0..8 {
        for motive in [
            inner.care(index),
            inner.want(index),
            inner.expect(index),
            inner.learned(index),
        ]
        .into_iter()
        .flatten()
        {
            let token = motive.token_str();
            if !token.is_empty() {
                out.push(token.to_string());
            }
        }
    }
    out
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
        let (first_clause, rest) = crate::mouth::clause_pair(&out.text);
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
            first_clause,
            rest,
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
        assert_eq!(hit.first_clause, "The kettle");
        assert_eq!(hit.rest, "is on the stove.");
        assert!(hit.sources.iter().any(|s| s == "card:kettle"));
        assert!(!hit.reply.contains("brain-unlinked"));
        let miss = mind.turn("who won the match").unwrap();
        assert_eq!(miss.reply, "I don't know.");
        assert_eq!(miss.first_clause, "I don't know.");
        assert!(miss.rest.is_empty());
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
