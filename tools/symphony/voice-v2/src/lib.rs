// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Voice V2 glue. One pipeline, fewest moving parts.
//!
//! Reference audio is cleaned, decoded into a profile, then a target feeling
//! is rendered, mixed with the room, read back by the same decoder, and
//! corrected once. The feeling is a point updated every 80 ms. The speaker's
//! baseline voice stays a separate record.
//!
//! Reused, not rewritten: [`galion_articulation`] (praatfan, in process) for
//! throat and mouth, and the existing `cbx speak` CLI when a local Chatterbox
//! snapshot is already on disk. This crate does not download weights.
//!
//! Emotion names are one set of 24 (Cowen, Elfenbein, Laukka, Keltner 2019,
//! American Psychologist). The 12 from Cowen et al. 2019, Nature Human
//! Behaviour, are a confidence tier (`reliable` in prosody), not a second
//! vocabulary. A spoken word may carry any of the 24. Neutral is the
//! speaker's own baseline, not one of the 24. Breath is physiology.

#![forbid(unsafe_code)]

pub mod audio;
pub mod decode;
pub mod device;
pub mod emotion;
pub mod fluid;
pub mod pipeline;
pub mod profile;
pub mod render;

pub use decode::{decode_samples, label_from_z};
pub use device::{choose, detect, DeviceChoice, DeviceKind};
pub use emotion::{
    parse_target, EmotionId, Feeling, ParsedTarget, ProsodyTier, Track, EXTENDED, RELIABLE,
};
pub use fluid::{
    BaselineVoice, FluidCore, InnerFactors, OuterFactors, Room, TargetState, TickFeeling,
};
pub use pipeline::{run_loop, speak, LoopIn, LoopReport, SpeakIn, SpeakOut};
pub use profile::VoiceFile;

/// Symphony clock. `LOCKS.toml` `clock.frame_ms`. Not a published feeling time constant.
pub const TICK_MS: u32 = 80;
pub const TICK_S: f64 = 0.080;
