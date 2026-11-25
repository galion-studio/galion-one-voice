// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Reply engine. The 7881 process runs `HorbmMind`, which calls
//! `horbm::Brain::turn`. [`UnlinkedBrain`] is the stand-in used by
//! [`crate::pipeline::FakePipeline`]: `turn` is an error, and `tick` does
//! not invent a mood. [`FakeReply`] exists so tests can drop a brain in
//! without speaking a comfort script.

use std::path::Path;
use std::time::Instant;

/// Arousal, valence, dominance. `RESTING` is the owner's triple.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn finite_or_resting(self) -> Self {
        if self.is_finite() {
            self
        } else {
            Self::RESTING
        }
    }
}

/// `intent_json` and `arm_json` are one JSON value each. This crate does not
/// name `horbm`'s enums, so the unlinked and fake brains send `null`.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent_json: String,
    pub arm_json: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
    /// Empathy weight from the HORBM turn. `0` is rest.
    pub empathy_e: f32,
    /// Prosody knobs for the voice. They follow `mood` and `empathy_e`.
    pub prosody_rate: f32,
    pub prosody_energy: f32,
    pub prosody_pitch_st: f32,
    pub prosody_pause_ms: f32,
    /// First spoken piece, copied from `horbm::TurnOut::first_clause`.
    /// Empty when this turn did not come from that brain.
    pub first_clause: String,
    /// Remainder after [`Self::first_clause`], copied from `horbm::TurnOut::rest`.
    pub rest: String,
}

pub trait ReplyEngine: Send {
    fn name(&self) -> &str;
    fn turn(&mut self, heard: &str) -> Result<TurnOut, String>;
    /// One call per 80 ms frame. `None` means the socket keeps `source: resting`.
    /// This method must not ease a pad toward a target.
    fn tick(&mut self) -> Option<Pad> {
        None
    }
}

/// `horbm::Brain` once it is linked. `tick` is separate from `turn`.
pub trait HorbmBrain: Send {
    fn turn(&mut self, text: &str) -> Result<TurnOut, String>;
    fn tick(&mut self);
}

pub trait OpenHorbm {
    type Brain: HorbmBrain;
    fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Result<Self::Brain, String>;
}

pub struct HorbmAdapter<B> {
    brain: B,
    last_tick: Option<Pad>,
}

impl<B> HorbmAdapter<B> {
    pub fn new(brain: B) -> Self {
        Self {
            brain,
            last_tick: None,
        }
    }
}

impl<B: HorbmBrain> ReplyEngine for HorbmAdapter<B> {
    fn name(&self) -> &str {
        "horbm"
    }

    fn turn(&mut self, heard: &str) -> Result<TurnOut, String> {
        let mut out = self.brain.turn(heard)?;
        out.mood = out.mood.finite_or_resting();
        if !out.propensity.is_finite() {
            out.propensity = 0.0;
        }
        Ok(out)
    }

    fn tick(&mut self) -> Option<Pad> {
        self.brain.tick();
        self.last_tick
    }
}

/// The process default. No cards, no spoken line, no mood path.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnlinkedBrain;

impl ReplyEngine for UnlinkedBrain {
    fn name(&self) -> &str {
        "unlinked"
    }

    fn turn(&mut self, _heard: &str) -> Result<TurnOut, String> {
        Err("brain-unlinked".into())
    }
}

/// Test double. One plain sentence. It does not echo the caller and it does
/// not move mood on `tick`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeReply;

impl ReplyEngine for FakeReply {
    fn name(&self) -> &str {
        "fake"
    }

    fn turn(&mut self, heard: &str) -> Result<TurnOut, String> {
        let started = Instant::now();
        if heard.trim().is_empty() {
            return Err("empty-text".into());
        }
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        Ok(TurnOut {
            reply: "The fence is loose.".into(),
            intent_json: "null".into(),
            arm_json: "null".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: Vec::new(),
            violations: 0,
            lookup_us: 0,
            took_us,
            mood: Pad::RESTING,
            empathy_e: 0.0,
            prosody_rate: 1.0,
            prosody_energy: 1.0,
            prosody_pitch_st: 0.0,
            prosody_pause_ms: 0.0,
            first_clause: String::new(),
            rest: String::new(),
        })
    }
}

const SCRIPTED_COMFORT: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "i'm here for you no matter what",
    "i feel that in my soul",
];

pub fn may_speak_piper(reply: &str, violations: usize) -> bool {
    violations == 0
        && !reply.trim().is_empty()
        && !is_scripted_comfort(reply)
        && !contains_bracket_tag(reply)
}

fn is_scripted_comfort(reply: &str) -> bool {
    let trimmed = reply.trim().trim_end_matches(['.', '!', '?']).trim();
    let lower = trimmed.to_ascii_lowercase();
    SCRIPTED_COMFORT.iter().any(|line| lower == *line)
}

fn contains_bracket_tag(reply: &str) -> bool {
    reply.contains('[') && reply.contains(']')
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub;

    impl HorbmBrain for Stub {
        fn turn(&mut self, text: &str) -> Result<TurnOut, String> {
            Ok(TurnOut {
                reply: format!("card:{text}"),
                intent_json: "null".into(),
                arm_json: "null".into(),
                propensity: 0.2,
                evidence: vec!["card:1".into()],
                sources: vec!["knowledge".into()],
                violations: 0,
                lookup_us: 40,
                took_us: 80,
                mood: Pad::RESTING,
                empathy_e: 0.0,
                prosody_rate: 1.0,
                prosody_energy: 1.0,
                prosody_pitch_st: 0.0,
                prosody_pause_ms: 0.0,
                first_clause: String::new(),
                rest: String::new(),
            })
        }
        fn tick(&mut self) {}
    }

    struct StubOpen;
    impl OpenHorbm for StubOpen {
        type Brain = Stub;
        fn open(_: &Path, _: &Path, _: &Path) -> Result<Stub, String> {
            Ok(Stub)
        }
    }

    #[test]
    fn resting_pad_matches_the_brief() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
    }

    #[test]
    fn unlinked_turn_is_an_error_and_tick_is_empty() {
        let mut brain = UnlinkedBrain;
        assert_eq!(brain.turn("hi").unwrap_err(), "brain-unlinked");
        assert!(brain.tick().is_none());
    }

    #[test]
    fn adapter_passes_turn_and_does_not_invent_a_tick_pad() {
        let brain = StubOpen::open(
            Path::new("memory.db"),
            Path::new("knowledge"),
            Path::new("cases"),
        )
        .unwrap();
        let mut engine = HorbmAdapter::new(brain);
        let out = engine.turn("hi").unwrap();
        assert_eq!(out.reply, "card:hi");
        assert_eq!(out.intent_json, "null");
        assert!(engine.tick().is_none());
    }

    #[test]
    fn fake_reply_is_speakable_and_not_comfort() {
        let out = FakeReply.turn("the kettle").unwrap();
        assert!(may_speak_piper(&out.reply, out.violations));
        assert!(!may_speak_piper("I hear you.", 0));
        assert!(!may_speak_piper("The fence is loose.", 1));
        assert!(!may_speak_piper("Come closer [whisper]", 0));
        assert!(FakeReply.tick().is_none());
    }
}
