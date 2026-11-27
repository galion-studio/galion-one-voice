// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Reply bridge for the configured persona (sector S04).
//!
//! Package `galion-sam-horbm`, directory `voice-stack/horbm-bridge`.
//! ASR text comes in. A [`ReplyEngine`] returns reply text, a [`Pad`] mood,
//! and source ids. [`speakable_chunks`] cuts that reply into clauses the
//! streaming TTS path can speak. The first clause goes out before the rest
//! of a long sentence. [`Session::tick`] runs once per 80 ms frame
//! between turns and does not start a turn. [`CardEngine`] steps one empathy
//! frame with no new words. [`FakeEngine`] holds its pad.
//!
//! Crate `horbm` is linked. One router turn is HORBM first, then Research
//! within 3 seconds, otherwise exactly `I don't know.` There is no S0 guess.
//! `Brain::open` in `galion-sam-orch`
//! still returns `horbm is not linked; Brain::open stays in crate horbm`.
//! That function is not the 7881 path. [`FakeEngine`] is the test double.
//! Feature `horbm` compiles [`HorbmEngine`] against crate `horbm`.
//! [`CardEngine`] is what the 7881 server calls.
//!
//! This crate does not synthesize audio and does not call a model API.
//! [`voice_ref_from_env`] reads the reference path for the configured persona:
//! `GALION_VOICE_REF` wins, and the old name is only a fallback. This crate
//! is not a member of the root workspace.

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
    contains_forbidden, speakable_chunks, split_first_clause, Chunker, SliceGrain, SpeakChunk,
    FIRST_SENTENCE_WORDS, FORBIDDEN, MIN_CLAUSE_WORDS, SPOKEN_CAP,
};
pub use engine::{Reply, ReplyEngine};
pub use fake::FakeEngine;
pub use galion_research_gateway::{
    ResearchClient, DEFAULT_BIND as RESEARCH_BIND, DEFAULT_PORT as RESEARCH_PORT, RESEARCH_CAP,
};
pub use horbm::{
    knowledge_dir_from_env, Brain, KnowledgeCards, LoadReport, MoodMeta, WINDOWS_KNOWLEDGE_DIR,
};
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

/// Env var for the configured persona's reference clip.
pub const VOICE_REF_ENV: &str = "GALION_VOICE_REF";

/// One resolved reference path, and at most one alias warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceRefChoice<'a> {
    pub value: Option<&'a str>,
    pub warning: Option<&'static str>,
}

/// [`VOICE_REF_ENV`] wins. The old variable is read only when the new
/// variable is unset. When both are set and differ, the new one wins and
/// `warning` is one line.
const LEGACY_VOICE_REF_ENV: &str = "GALION_SAMANTHA_REF";

const FALLBACK_WARNING: &str = "GALION_SAMANTHA_REF is a fallback; set GALION_VOICE_REF";

const DIFFER_WARNING: &str = "GALION_VOICE_REF wins over GALION_SAMANTHA_REF";

/// Pick the reference path from the new variable and the old one.
///
/// Empty strings count as unset. This function does not read the process
/// environment and does not touch the filesystem.
pub fn voice_ref_choice<'a>(
    new_name: Option<&'a str>,
    old_name: Option<&'a str>,
) -> VoiceRefChoice<'a> {
    let new_name = nonempty(new_name);
    let old_name = nonempty(old_name);
    let (value, warning) = match (new_name, old_name) {
        (None, None) => (None, None),
        (Some(value), None) => (Some(value), None),
        (None, Some(value)) => (Some(value), Some(FALLBACK_WARNING)),
        (Some(value), Some(old)) if value == old => (Some(value), None),
        (Some(value), Some(_)) => (Some(value), Some(DIFFER_WARNING)),
    };
    VoiceRefChoice { value, warning }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    match value {
        Some(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}

/// Path named by the environment. Does not stat the path and does not open it.
///
/// A stat here ran on the host turn (`Brain::turn` -> this helper) and missed
/// the socket's turn deadline. The mouth still checks that the path is a file.
/// [`VOICE_REF_ENV`] wins. One fallback warning is printed on this thread,
/// once, with no spawned thread and no cache lock.
pub fn voice_ref_from_env() -> Option<std::path::PathBuf> {
    let new_raw = nonempty_os(VOICE_REF_ENV);
    let old_raw = nonempty_os(LEGACY_VOICE_REF_ENV);
    let (chosen, warning) = match (&new_raw, &old_raw) {
        (None, None) => return None,
        (Some(value), None) => (value, None),
        (None, Some(value)) => (value, Some(FALLBACK_WARNING)),
        (Some(value), Some(old)) if value == old => (value, None),
        (Some(value), Some(_)) => (value, Some(DIFFER_WARNING)),
    };
    if let Some(line) = warning {
        warn_line(line);
    }
    Some(std::path::PathBuf::from(chosen))
}

fn warn_line(line: &'static str) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    eprintln!("{line}");
}

fn nonempty_os(key: &str) -> Option<std::ffi::OsString> {
    let raw = std::env::var_os(key)?;
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}
