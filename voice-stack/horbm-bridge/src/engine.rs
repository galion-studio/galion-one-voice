// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The reply contract the rest of the voice stack calls.
//!
//! A turn is text in, reply text plus mood plus sources out. [`ReplyEngine::tick`]
//! is one 80 ms frame. It does not ask a new question and it does not speak.
//! The host still calls it on the heartbeat between turns. [`crate::CardEngine`]
//! steps empathy with no new words, so a raised mood eases toward rest.
//! [`crate::FakeEngine`] holds the pad.

use crate::pad::Pad;

/// What one turn hands to TTS and to the feeling band.
///
/// `text` is the brain's line, tags and all. Speak [`crate::speakable_chunks`]
/// of it. `sources` are card ids from the brain (empty when the fake engine
/// had nothing to answer). The timing fields are the brain's own counters
/// when the HORBM adapter is linked; the fake engine fills `took_us` from
/// the local call and leaves the lookup at 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub mood: Pad,
    pub sources: Vec<String>,
    pub evidence: Vec<String>,
    pub propensity: f32,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
}

/// Text in, spoken reply and mood out.
///
/// Implementors must be cheap on [`Self::tick`]: a few arithmetic ops, no
/// file, no socket, no model call. [`Self::turn`] may do the brain lookup.
pub trait ReplyEngine {
    fn turn(&mut self, text: &str) -> Reply;
    /// One 80 ms frame. Does not start a turn and does not speak.
    fn tick(&mut self) -> Pad;
    fn mood(&self) -> Pad;
}
