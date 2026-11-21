// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Deterministic stand-in for `horbm::Brain`.
//!
//! It repeats the caller's words and moves [`Pad`] only so tests can see the
//! tick. It does not retrieve cards and it does not write a comfort line.
//! A `!` in the input sets a fixed brighter pad. A `?` sets a fixed quieter
//! one. Anything else keeps the pad the tick has already decayed.

use std::time::Instant;

use crate::engine::{Reply, ReplyEngine};
use crate::pad::Pad;

#[derive(Debug)]
pub struct FakeEngine {
    pad: Pad,
    turns: u32,
}

impl Default for FakeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeEngine {
    pub fn new() -> Self {
        Self {
            pad: Pad::RESTING,
            turns: 0,
        }
    }

    pub fn turns(&self) -> u32 {
        self.turns
    }
}

impl ReplyEngine for FakeEngine {
    fn turn(&mut self, text: &str) -> Reply {
        let started = Instant::now();
        let trimmed = text.trim();
        self.turns = self.turns.saturating_add(1);
        if trimmed.is_empty() {
            return Reply {
                text: String::new(),
                mood: self.pad,
                sources: Vec::new(),
                evidence: Vec::new(),
                propensity: 0.0,
                violations: 0,
                lookup_us: 0,
                took_us: elapsed_us(started),
            };
        }
        self.pad = pad_for(trimmed, self.pad);
        Reply {
            text: echo(trimmed),
            mood: self.pad,
            sources: vec!["fake:echo".into()],
            evidence: Vec::new(),
            propensity: 0.0,
            violations: 0,
            lookup_us: 0,
            took_us: elapsed_us(started),
        }
    }

    fn tick(&mut self) -> Pad {
        self.pad = self.pad.advance_frame();
        self.pad
    }

    fn mood(&self) -> Pad {
        self.pad
    }
}

fn echo(text: &str) -> String {
    if text.ends_with(['.', '!', '?']) {
        text.to_string()
    } else {
        let mut s = String::with_capacity(text.len() + 1);
        s.push_str(text);
        s.push('.');
        s
    }
}

fn pad_for(text: &str, current: Pad) -> Pad {
    if text.contains('!') {
        Pad::new(0.7, 0.55, 0.5)
    } else if text.contains('?') {
        Pad::new(0.4, 0.25, 0.4)
    } else {
        current
    }
}

fn elapsed_us(started: Instant) -> u32 {
    let us = started.elapsed().as_micros();
    u32::try_from(us).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_keeps_their_words_and_names_the_fake_source() {
        let mut eng = FakeEngine::new();
        let reply = eng.turn("  the kettle is on  ");
        assert_eq!(reply.text, "the kettle is on.");
        assert_eq!(reply.sources, vec!["fake:echo".to_string()]);
        assert!(reply.evidence.is_empty());
        assert_eq!(reply.mood, Pad::RESTING);
        assert_eq!(eng.turns(), 1);
        assert!(reply.text.to_ascii_lowercase().contains("kettle"));
        assert!(!reply.text.to_ascii_lowercase().contains("i hear you"));
    }

    #[test]
    fn bang_and_question_set_a_fixed_pad_and_blank_does_not() {
        let mut eng = FakeEngine::new();
        let blank = eng.turn("   ");
        assert!(blank.text.is_empty());
        assert!(blank.sources.is_empty());
        assert_eq!(blank.mood, Pad::RESTING);
        let loud = eng.turn("hey!");
        assert_eq!(loud.text, "hey!");
        assert_eq!(loud.mood, Pad::new(0.7, 0.55, 0.5));
        let ask = eng.turn("where?");
        assert_eq!(ask.mood, Pad::new(0.4, 0.25, 0.4));
    }
}
