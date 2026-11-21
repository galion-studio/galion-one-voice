// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Samantha's reply bridge (sector S04).
//!
//! Package `galion-sam-horbm`, directory `voice-stack/horbm-bridge`.
//! ASR text comes in. A [`ReplyEngine`] returns reply text, a [`Pad`] mood,
//! and source ids. [`speakable_chunks`] cuts that reply into sentences the
//! streaming TTS path can speak. [`Session::tick`] runs once per 80 ms frame
//! between turns and does not start a turn. [`CardEngine`] steps one empathy
//! frame with no new words. [`FakeEngine`] holds its pad.
//!
//! Crate `horbm` is linked. One turn is empathy, then cards, then the
//! research gateway, then `I don't know.` `Brain::open` in `galion-sam-orch`
//! still returns `horbm is not linked; Brain::open stays in crate horbm`.
//! That function is not the 7881 path. [`FakeEngine`] is the test double.
//! Feature `horbm` compiles [`HorbmEngine`] against crate `horbm`.
//! [`CardEngine`] is what the 7881 server calls.
//!
//! This crate does not synthesize audio, does not read `GALION_SAMANTHA_REF`,
//! and does not call a model API. It is not a member of the root workspace.

mod cards;
mod chunks;
mod engine;
mod fake;
mod index;
mod measure;
mod pad;
mod research;
mod router;
mod session;

#[cfg(feature = "horbm")]
mod linked;

pub use cards::{CardBrain, CardEngine, UNKNOWN_REPLY};
pub use chunks::{
    contains_forbidden, speakable_chunks, Chunker, SpeakChunk, FIRST_SENTENCE_WORDS, FORBIDDEN,
    SPOKEN_CAP,
};
pub use engine::{Reply, ReplyEngine};
pub use fake::FakeEngine;
pub use galion_research_gateway::{
    ResearchClient, DEFAULT_BIND as RESEARCH_BIND, DEFAULT_PORT as RESEARCH_PORT, RESEARCH_CAP,
};
pub use horbm::{Brain, MoodMeta};
pub use index::{CardError, CardLoader, CardMatch, LoadedCard, MemoryCards, GAP_ADVICE, MAX_HITS};
pub use measure::{time_ns, SampleNs};
pub use pad::{Pad, FRAME_MS};
pub use research::route;
pub use router::{
    is_deep_question, GapEntry, GapReason, ReasoningRouter, ResearchAnswer, ResearchPort,
    RouteKind, RouteOutcome,
};
pub use session::{MoodTick, Session, SpokenTurn};

#[cfg(feature = "horbm")]
pub use linked::HorbmEngine;

/// Path of the real crate, relative to this crate's `Cargo.toml`.
///
/// Path of crate `horbm`, relative to this crate's `Cargo.toml`.
pub const HORBM_LANDING_PATH: &str = "../../horbm/crates/horbm";

/// `true` because crate `horbm` is a dependency of this bridge.
pub fn horbm_linked() -> bool {
    true
}
