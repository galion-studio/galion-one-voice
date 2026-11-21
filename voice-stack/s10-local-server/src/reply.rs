// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Reply engine. The real brain is `horbm::Brain` from the `horbm` crate,
//! which is not in this repo. `Brain::open(memory_db, knowledge_dir, cases)`
//! and `brain.turn(&str) -> TurnOut` are the surface this sector codes against.
//! `horbm` depends on `horbm-memory` from `galion-studio/galion-knowledge`
//! revision `761a7f225304fc010379219e5b88c6669dd48f40`. This file does not
//! fetch that tree and does not reimplement retrieval.

use serde::Serialize;
use std::path::Path;
use std::time::Instant;

/// Pleasure-arousal-dominance. Field order matches the brief:
/// arousal, valence, dominance. `RESTING` is the idle pad.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
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

    /// Finite values only. Arousal and dominance stay in 0..=1.
    /// Valence stays in -1..=1. A non-finite channel falls back to rest
    /// for that channel, then the range clamp still applies.
    pub fn sanitised(self) -> Self {
        Self {
            arousal: clean(self.arousal, 0.0, 1.0, Self::RESTING.arousal),
            valence: clean(self.valence, -1.0, 1.0, Self::RESTING.valence),
            dominance: clean(self.dominance, 0.0, 1.0, Self::RESTING.dominance),
        }
    }

    pub fn approach(self, target: Self, step: f32) -> Self {
        let step = if step.is_finite() {
            step.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mix = |a: f32, b: f32| a + (b - a) * step;
        Self {
            arousal: mix(self.arousal, target.arousal),
            valence: mix(self.valence, target.valence),
            dominance: mix(self.dominance, target.dominance),
        }
        .sanitised()
    }
}

fn clean(x: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        fallback
    }
}

/// What `horbm::Brain::turn` returns. Names match the crate.
#[derive(Debug, Clone, PartialEq, Serialize)]
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

/// The mouth of the stack calls this. `FakeReply` is the stand-in until
/// [`HorbmAdapter`] wraps a real `horbm::Brain`.
pub trait ReplyEngine: Send {
    fn name(&self) -> &str;
    fn turn(&mut self, heard: &str) -> TurnOut;
}

/// What `Brain::turn` must look like so the adapter can wrap it
/// without this crate depending on `horbm`.
pub trait HorbmBrain: Send {
    fn turn(&mut self, text: &str) -> TurnOut;
}

/// `horbm::Brain::open(memory_db, knowledge_dir, cases)`.
/// A later binary implements this and passes the brain to [`HorbmAdapter`].
/// This sector never opens the database itself.
pub trait OpenHorbm {
    type Brain: HorbmBrain;
    fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Result<Self::Brain, String>;
}

/// Thin wrapper. `name()` is `horbm` so the socket can say which mind is on.
pub struct HorbmAdapter<B> {
    brain: B,
}

impl<B> HorbmAdapter<B> {
    pub fn new(brain: B) -> Self {
        Self { brain }
    }

    pub fn into_inner(self) -> B {
        self.brain
    }
}

impl<B: HorbmBrain> ReplyEngine for HorbmAdapter<B> {
    fn name(&self) -> &str {
        "horbm"
    }

    fn turn(&mut self, heard: &str) -> TurnOut {
        let mut out = self.brain.turn(heard);
        out.mood = out.mood.sanitised();
        if !out.propensity.is_finite() {
            out.propensity = 0.0;
        }
        out
    }
}

/// Unwired mind. One short plain line that repeats the heard words.
/// It is not Samantha's brain and it does not perform a comfort script.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeReply;

impl ReplyEngine for FakeReply {
    fn name(&self) -> &str {
        "fake"
    }

    fn turn(&mut self, heard: &str) -> TurnOut {
        let started = Instant::now();
        let heard = clip_words(heard, 120);
        let reply = if heard.is_empty() {
            String::new()
        } else {
            format!("Okay. {heard}")
        };
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        TurnOut {
            reply,
            intent: "reply".into(),
            arm: "fake".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: Vec::new(),
            violations: 0,
            lookup_us: 0,
            took_us,
            mood: Pad::RESTING,
        }
    }
}

fn clip_words(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for w in s.split_whitespace() {
        let extra = if out.is_empty() { w.len() } else { w.len() + 1 };
        if out.len() + extra > max_chars {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubBrain;

    impl HorbmBrain for StubBrain {
        fn turn(&mut self, text: &str) -> TurnOut {
            TurnOut {
                reply: format!("card:{text}"),
                intent: "lookup".into(),
                arm: "retrieve".into(),
                propensity: 0.2,
                evidence: vec!["card:1".into()],
                sources: vec!["knowledge".into()],
                violations: 0,
                lookup_us: 40,
                took_us: 80,
                mood: Pad::RESTING,
            }
        }
    }

    struct StubOpen;

    impl OpenHorbm for StubOpen {
        type Brain = StubBrain;
        fn open(
            memory_db: &Path,
            knowledge_dir: &Path,
            cases: &Path,
        ) -> Result<Self::Brain, String> {
            let _ = (memory_db, knowledge_dir, cases);
            Ok(StubBrain)
        }
    }

    #[test]
    fn resting_pad_matches_the_brief() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
    }

    #[test]
    fn adapter_passes_the_turn_through() {
        let brain = StubOpen::open(
            Path::new("memory.db"),
            Path::new("knowledge"),
            Path::new("cases"),
        )
        .unwrap();
        let mut engine = HorbmAdapter::new(brain);
        assert_eq!(engine.name(), "horbm");
        let out = engine.turn("hi");
        assert_eq!(out.reply, "card:hi");
        assert_eq!(out.sources, vec!["knowledge".to_string()]);
        assert_eq!(out.mood, Pad::RESTING);
        assert_eq!(out.lookup_us, 40);
        let _brain: StubBrain = engine.into_inner();
    }

    #[test]
    fn fake_reply_is_short_and_not_a_comfort_script() {
        let mut fake = FakeReply;
        let out = fake.turn("  the   kettle   is   loud  ");
        assert_eq!(out.reply, "Okay. the kettle is loud");
        assert_eq!(out.mood, Pad::RESTING);
        assert!(out.violations == 0);
        let banned = [
            "I hear you",
            "That sounds so hard",
            "I'm here for you",
            "I feel that in my soul",
        ];
        for line in banned {
            assert!(!out.reply.contains(line), "{line}");
        }
        assert!(out.reply.chars().count() < 160);
    }

    #[test]
    fn fake_reply_clips_a_long_line() {
        let long = "word ".repeat(80);
        let out = FakeReply.turn(&long);
        let body = out.reply.strip_prefix("Okay. ").unwrap();
        assert!(body.chars().count() <= 120);
        assert!(!body.ends_with(' '));
    }
}
