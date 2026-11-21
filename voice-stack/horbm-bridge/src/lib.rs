// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Samantha's reply bridge (sector S04).
//!
//! Package `galion-sam-horbm`, directory `voice-stack/horbm-bridge`.
//! ASR text comes in. A [`ReplyEngine`] returns reply text, a [`Pad`] mood,
//! and source ids. [`speakable_chunks`] cuts that reply into sentences the
//! streaming TTS path can speak. [`Session::tick`] runs once per 80 ms frame
//! between turns. `horbm` has no `tick` in the brief, so this tick is empty.
//! It does not synthesize a mood trajectory.
//!
//! The real brain is the `horbm` crate (`Brain::open`, `Brain::turn`). It is
//! not in this tree yet. [`FakeEngine`] is the test double. Feature `horbm`
//! compiles [`HorbmEngine`] against whatever crate the `horbm` path dependency
//! points at. That is `seam/horbm` until `../../horbm/crates/horbm` lands.
//!
//! This crate does not synthesize audio, does not read `GALION_SAMANTHA_REF`,
//! and does not call a model API. It is not a member of the root workspace.

mod cards;
mod chunks;
mod engine;
mod fake;
mod measure;
mod pad;
mod research;
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
pub use measure::{time_ns, SampleNs};
pub use pad::{Pad, FRAME_MS};
pub use session::{MoodTick, Session, SpokenTurn};

#[cfg(feature = "horbm")]
pub use linked::HorbmEngine;

/// Path of the real crate, relative to this crate's `Cargo.toml`.
///
/// Cargo 1.83 loads an optional path dependency even when its feature is off,
/// and this directory is not in the tree yet. The feature links `seam/horbm`
/// until this path exists. `build.rs` warns once the landing `Cargo.toml` is
/// on disk.
pub const HORBM_LANDING_PATH: &str = "../../horbm/crates/horbm";

/// `true` when this build was compiled with `--features horbm`.
pub fn horbm_linked() -> bool {
    cfg!(feature = "horbm")
}
