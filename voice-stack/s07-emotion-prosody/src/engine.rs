// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Adapter surface for `horbm::Brain`.
//!
//! The real crate is not on this tree. [`ReplyEngine`] is the call this
//! sector needs: one turn in, one [`TurnOut`] out. Only [`TurnOut::mood`]
//! changes the voice. [`FakeReplyEngine`] stands in for tests. It does not
//! retrieve cards and it does not synthesise speech.

use crate::pad::Pad;

/// What a reply engine returns. Field names match the HORBM `TurnOut` the
/// stack will consume. This struct is not a brain.
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

/// `Brain::turn` without opening a memory database.
pub trait ReplyEngine {
    fn turn(&mut self, heard: &str) -> TurnOut;
}

/// Fixed mood and fixed reply. The heard text is ignored on purpose.
#[derive(Clone, Debug)]
pub struct FakeReplyEngine {
    pub reply: String,
    pub mood: Pad,
}

impl FakeReplyEngine {
    pub fn resting() -> Self {
        Self {
            reply: "Hi. I'm here.".to_string(),
            mood: Pad::RESTING,
        }
    }
}

impl ReplyEngine for FakeReplyEngine {
    fn turn(&mut self, _heard: &str) -> TurnOut {
        TurnOut {
            reply: self.reply.clone(),
            intent: "fake".to_string(),
            arm: "none".to_string(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: Vec::new(),
            violations: 0,
            lookup_us: 0,
            took_us: 0,
            mood: self.mood,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ProsodyClock;
    use crate::ReplyEngine;

    #[test]
    fn the_reply_string_does_not_change_the_voice() {
        let mood = Pad {
            arousal: 0.62,
            valence: 0.58,
            dominance: 0.52,
        };
        let a = run("hello", mood);
        let b = run("I am so happy [giggle] [whisper]!!!", mood);
        assert_eq!(a, b);
    }

    fn run(reply: &str, mood: Pad) -> crate::SpeakingParams {
        let mut engine = FakeReplyEngine {
            reply: reply.to_string(),
            mood,
        };
        let turn = engine.turn("anything");
        assert_eq!(turn.reply, reply);
        assert_eq!(turn.mood, mood);
        let mut clock = ProsodyClock::new();
        clock.follow(turn.mood);
        let mut params = clock.tick();
        for _ in 0..40 {
            params = clock.tick();
        }
        params
    }

    #[test]
    fn fake_rest_matches_pad_rest() {
        let mut engine = FakeReplyEngine::resting();
        let turn = engine.turn("hi");
        assert_eq!(turn.mood, Pad::RESTING);
        assert!(turn.violations == 0);
    }
}
