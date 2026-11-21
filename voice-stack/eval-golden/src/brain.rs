// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Reply path Samantha's golden scripts call.
//!
//! The real brain is crate `horbm`, which is not in this tree:
//! `Brain::open(memory_db, knowledge_dir, cases)` then
//! `brain.turn(&str) -> TurnOut`. This module is the trait that crate will
//! satisfy, plus a fake used by tests. It does not retrieve cards, does not
//! open a database, and does not invent evidence.

use crate::gate::STREAM_BAR_MS;

/// HORBM pad. Field order is arousal, valence, dominance.
/// `RESTING` is the swarm brief `{0.3, 0.25, 0.5}`.
/// Samantha's persona rest in `galion-emotion` is a different triple
/// (valence, arousal, dominance) and is not copied here.
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

/// One `horbm` turn. `arm` is `"fake"` for the stand-in.
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

/// What `horbm::Brain` will implement. One method, the turn.
pub trait ReplyEngine {
    fn turn(&mut self, user: &str) -> TurnOut;
}

/// Mood advances on the 80 ms heartbeat, one lerp step per tick.
/// This is the fake clock, not HORBM's mood model.
#[derive(Clone, Debug)]
pub struct MoodClock {
    pub pad: Pad,
    target: Pad,
    pub ticks: u32,
}

impl MoodClock {
    pub const TICK_MS: u32 = STREAM_BAR_MS as u32;

    pub fn resting() -> Self {
        Self {
            pad: Pad::RESTING,
            target: Pad::RESTING,
            ticks: 0,
        }
    }

    pub fn set_target(&mut self, target: Pad) {
        self.target = target;
    }

    pub fn advance(&mut self) {
        const STEP: f32 = 0.2;
        self.pad.arousal += STEP * (self.target.arousal - self.pad.arousal);
        self.pad.valence += STEP * (self.target.valence - self.pad.valence);
        self.pad.dominance += STEP * (self.target.dominance - self.pad.dominance);
        self.ticks = self.ticks.saturating_add(1);
    }

    pub fn elapsed_ms(&self) -> u32 {
        self.ticks.saturating_mul(Self::TICK_MS)
    }
}

impl Default for MoodClock {
    fn default() -> Self {
        Self::resting()
    }
}

/// Scripted Samantha lines for the golden set. Unknown text is an admission,
/// not a guess. `arm` is always `"fake"`.
#[derive(Clone, Debug)]
pub struct FakeReplyEngine {
    pub mood: MoodClock,
}

impl Default for FakeReplyEngine {
    fn default() -> Self {
        Self {
            mood: MoodClock::resting(),
        }
    }
}

impl FakeReplyEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReplyEngine for FakeReplyEngine {
    fn turn(&mut self, user: &str) -> TurnOut {
        let (reply, intent) = scripted(user);
        self.mood.set_target(target_for(user));
        self.mood.advance();
        TurnOut {
            reply,
            intent,
            arm: "fake".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: Vec::new(),
            violations: 0,
            lookup_us: 0,
            took_us: MoodClock::TICK_MS.saturating_mul(1000),
            mood: self.mood.pad,
        }
    }
}

fn scripted(user: &str) -> (String, String) {
    let key = user.trim();
    let (reply, intent) = match key {
        "Hey." => ("Hi. I'm here.", "greeting"),
        "I finally fixed the garden gate." => ("You fixed it. How does it sit now?", "ack"),
        "It sticks a little on the left." => {
            ("The left side. Does it catch on the latch?", "follow")
        }
        "Tell me the whole story of the gate." => {
            ("You fixed the gate. The left side still sticks.", "follow")
        }
        "Stop. Wait." => ("Okay.", "stop"),
        "Who won the match I did not tell you about?" => {
            ("I don't know. I wasn't there.", "unknown")
        }
        "Today was heavy. I don't want advice." => ("Okay. I'll sit with that.", "check_in"),
        _ => ("I don't know.", "unknown"),
    };
    (reply.to_string(), intent.to_string())
}

fn target_for(user: &str) -> Pad {
    let u = user.to_lowercase();
    if u.contains("heavy") {
        Pad {
            arousal: 0.22,
            valence: 0.08,
            dominance: 0.42,
        }
    } else if u.contains("stop") {
        Pad::RESTING
    } else if u.contains("fixed") || u == "hey." {
        Pad {
            arousal: 0.34,
            valence: 0.4,
            dominance: 0.5,
        }
    } else {
        Pad::RESTING
    }
}

/// The real `Brain::open` is not linked. Calling this does not touch a database.
pub fn open_horbm(
    memory_db: &str,
    knowledge_dir: &str,
    cases: &str,
) -> Result<FakeReplyEngine, String> {
    let _ = (memory_db, knowledge_dir, cases);
    Err("crate `horbm` is not in this tree. Brain::open was not called. No retrieval ran. Use FakeReplyEngine until the crate is linked.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_does_not_pretend_the_brain_is_present() {
        let err = open_horbm("memory.db", "knowledge", "cases").unwrap_err();
        assert!(err.contains("horbm"));
        assert!(err.contains("No retrieval"));
    }

    #[test]
    fn one_tick_is_80_ms_and_check_in_moves_valence() {
        assert_eq!(MoodClock::TICK_MS, 80);
        let mut engine = FakeReplyEngine::new();
        assert_eq!(engine.mood.pad, Pad::RESTING);
        let turn = engine.turn("Today was heavy. I don't want advice.");
        assert!(turn.mood.valence < Pad::RESTING.valence);
        assert_eq!(engine.mood.ticks, 1);
        assert_eq!(engine.mood.elapsed_ms(), 80);
        assert_eq!(turn.arm, "fake");
        assert!(turn.evidence.is_empty());
        assert!(turn.sources.is_empty());
        assert_eq!(turn.lookup_us, 0);
    }

    #[test]
    fn unknown_user_text_is_an_admission() {
        let mut engine = FakeReplyEngine::new();
        let turn = engine.turn("What is the password of the building?");
        assert!(turn.reply.to_lowercase().contains("don't know"));
        assert!(turn.evidence.is_empty());
    }
}
