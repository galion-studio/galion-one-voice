// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Versioned frame contract between Inner Life and Mouth-and-Expressions.
//!
//! One document is either an intent stream (Inner Life to MnE) or a read-back
//! stream (MnE to Inner Life). Frames sit on the 80 ms Symphony clock
//! (`LOCKS.toml` `clock.frame_ms`). The feeling is a continuous point.
//! A Cowen 2019 name is an optional pin. Neutral is the speaker baseline,
//! not a twenty-fifth category. Breath is a body slot, not a pin.
//!
//! This crate does not decode audio, render a voice, or plan the lungs.
//! Those stay in their own crates. The field names line up with them.

pub mod binary;
pub mod mirror;
pub mod model;
pub mod names;
pub mod validate;
pub mod wire;

pub use model::{
    BaselinePoint, BodyOrigin, BodyState, BreathSegment, Confidence, DocKind, Document, Feeling,
    InnerFactors, Intent, IntentFrame, LungsState, Moment, MouthState, NoseState, PersonaRef, Pin,
    ReadbackFrame, TextSpan, ThroatState, ValueOrigin,
};
pub use names::{resolve_name, CowenName, ProsodyTier, ResolvedName, Track, COWEN, RELIABLE};
pub use validate::validate;

/// Wire schema name. JSON and TOML documents carry this string.
pub const SCHEMA: &str = "galion-mne-contract";

/// Binary magic is `GMN` plus this version as an ASCII digit.
pub const SCHEMA_VERSION: u16 = 1;

/// Symphony heartbeat. Not a published feeling time constant.
pub const FRAME_MS: u16 = 80;
