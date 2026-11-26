// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local HORBM mind. A turn is [`galion_sam_horbm::CardEngine::turn_with_ear`].
//! Typed text passes no listener point. A live mic utterance passes that
//! utterance's point. The brain speaks a retrieval hit. `Idle` is `remember`
//! denied: cards answer, and the clause fixture covers a card miss. A granted
//! retrieval miss is `I don't know.` until the engine speaks a loaded card.
//! That card read does not take the retrieval cell lock again. A deep miss
//! on the idle path may ask the research gateway.
//! [`HorbmMind::take_gap_for`] and [`HorbmMind::research_follow_up`] are what
//! the turn log reads after the reply. A follow-up is not spoken.
//!
//! Remember and learn are the words in [`crate::host::HostConfig`]. This
//! process does not call [`std::env::set_var`]. `grant` opens the person
//! diary at an absolute path. Any other word leaves it closed. `Brain::open`
//! in `voice-stack/pipeline-orchestrator` still returns `horbm is not linked`.
//! That is not this process. There is no knowledge-ro listener in this crate.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

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

/// What one turn handed to `Brain::turn_with_ear`. Production leaves this unset.
pub(crate) struct EarTrace {
    last_point: Mutex<Option<(f32, f32, f32)>>,
    with_point: AtomicU32,
    without: AtomicU32,
    last_turn: Mutex<Option<TurnOut>>,
}

impl EarTrace {
    pub(crate) fn new() -> Self {
        Self {
            last_point: Mutex::new(None),
            with_point: AtomicU32::new(0),
            without: AtomicU32::new(0),
            last_turn: Mutex::new(None),
        }
    }

    fn record(&self, ear: Option<horbm::Heard>, turn: &TurnOut) {
        match ear {
            Some(point) => {
                *self
                    .last_point
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) =
                    Some((point.valence, point.arousal, point.confidence));
                self.with_point.fetch_add(1, Ordering::Relaxed);
            }
            None => {
                self.without.fetch_add(1, Ordering::Relaxed);
            }
        }
        *self.last_turn.lock().unwrap_or_else(|err| err.into_inner()) = Some(turn.clone());
    }

    pub(crate) fn last_point(&self) -> Option<(f32, f32, f32)> {
        *self
            .last_point
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn with_point(&self) -> u32 {
        self.with_point.load(Ordering::Relaxed)
    }

    pub(crate) fn without(&self) -> u32 {
        self.without.load(Ordering::Relaxed)
    }

    pub(crate) fn last_turn(&self) -> Option<TurnOut> {
        self.last_turn
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

pub struct HorbmMind {
    engine: CardEngine,
    trace: Option<Arc<EarTrace>>,
    /// Gap queue already offered to a turn. The next take returns only a
    /// newer entry, so one miss is logged once.
    seen_gaps: Vec<horbm::Gap>,
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
            trace: None,
            seen_gaps: Vec::new(),
        }
    }

    pub(crate) fn with_trace(mut self, trace: Arc<EarTrace>) -> Self {
        self.trace = Some(trace);
        self
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

    /// Newest gap whose `heard` equals `heard` trimmed, at most once.
    ///
    /// The queue is [`horbm::Brain::gaps`]. A second call returns [`None`]
    /// until a later turn records another gap for that same line. This does
    /// not remove the brain's copy and it does not block.
    pub fn take_gap_for(&mut self, heard: &str) -> Option<horbm::Gap> {
        let query = heard.trim();
        if query.is_empty() {
            return None;
        }
        let current = self.engine.brain().gaps().to_vec();
        let start = fresh_gap_index(&self.seen_gaps, &current);
        let found = current[start..]
            .iter()
            .rev()
            .find(|gap| gap.heard == query)
            .cloned();
        self.seen_gaps = current;
        found
    }

    /// Research result that has finished since the last take.
    ///
    /// Pass-through to [`horbm::Brain::research_follow_up`]. It does not
    /// block, and it does not change the reply already returned.
    pub fn research_follow_up(&mut self) -> Option<horbm::ResearchFollowUp> {
        self.engine.brain_mut().research_follow_up()
    }
}

/// Index in `current` where entries newer than `seen` begin.
///
/// The brain drops the oldest gap at the cap and may replace one in place
/// when a follow-up finishes. The longest suffix of `seen` that is a prefix
/// of `current` is the part already offered.
fn fresh_gap_index(seen: &[horbm::Gap], current: &[horbm::Gap]) -> usize {
    let max = seen.len().min(current.len());
    for kept in (0..=max).rev() {
        if seen[seen.len() - kept..] == current[..kept] {
            return kept;
        }
    }
    0
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
        self.turn_heard(heard, None)
    }

    fn turn_heard(&mut self, heard: &str, ear: Option<horbm::Heard>) -> Result<TurnOut, String> {
        let out = self.engine.turn_with_ear(heard, ear);
        let meta = self.engine.mood_meta();
        let (first_clause, rest) = crate::mouth::clause_pair(&out.reply);
        let mapped = TurnOut {
            reply: out.reply,
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
        };
        if let Some(trace) = &self.trace {
            trace.record(ear, &mapped);
        }
        Ok(mapped)
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
    fn granted_remember_still_speaks_the_shipped_kettle_card() {
        let root = std::env::temp_dir().join(format!(
            "galion-grant-kettle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let mut cfg = HostConfig::deny_under(&root);
        cfg.remember.word = "grant".into();
        cfg.remember.granted = true;
        cfg.retrieval.remember = true;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge");
        let mut mind = HorbmMind::open_configured(&dir, &cfg);
        let hit = mind.turn("where is the kettle").unwrap();
        assert_eq!(hit.reply, "The kettle is on the stove.");
        assert_eq!(hit.first_clause, "The kettle");
        assert_eq!(hit.rest, "is on the stove.");
        assert!(
            hit.sources
                .iter()
                .any(|source| source == "card:kettle" || source.starts_with("retrieve:")),
            "{:?}",
            hit.sources
        );
        let miss = mind.turn("who won the match").unwrap();
        assert_eq!(miss.reply, "I don't know.");
        drop(mind);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn text_turn_matches_a_turn_with_no_ear_point() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge");
        let mut mind = HorbmMind::open(&dir);
        let via_turn = mind.turn("where is the kettle").unwrap();
        let via_ear = mind.turn_heard("where is the kettle", None).unwrap();
        assert_eq!(via_turn.reply, via_ear.reply);
        assert_eq!(via_turn.reply, "The kettle is on the stove.");
        assert_eq!(via_turn.sources, via_ear.sources);
        assert_eq!(via_turn.evidence, via_ear.evidence);
        assert_eq!(via_turn.first_clause, via_ear.first_clause);
        assert_eq!(via_turn.rest, via_ear.rest);
        assert_eq!(via_turn.intent_json, via_ear.intent_json);
        assert_eq!(via_turn.arm_json, via_ear.arm_json);
        assert_eq!(via_turn.mood, via_ear.mood);
        assert_eq!(via_turn.violations, via_ear.violations);
        assert_eq!(via_turn.empathy_e.to_bits(), via_ear.empathy_e.to_bits());
        assert_eq!(
            via_turn.prosody_rate.to_bits(),
            via_ear.prosody_rate.to_bits()
        );
        assert_eq!(
            via_turn.prosody_energy.to_bits(),
            via_ear.prosody_energy.to_bits()
        );
        assert_eq!(
            via_turn.prosody_pitch_st.to_bits(),
            via_ear.prosody_pitch_st.to_bits()
        );
        assert_eq!(
            via_turn.prosody_pause_ms.to_bits(),
            via_ear.prosody_pause_ms.to_bits()
        );
        assert_eq!(via_turn.propensity.to_bits(), via_ear.propensity.to_bits());
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
