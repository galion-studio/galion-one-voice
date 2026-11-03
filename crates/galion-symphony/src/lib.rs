// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Symphony load path for Galion Voice V2.
//!
//! Clone safetensors are an interchange. The runtime objects are Galion `.gq`
//! tensors plus a `.gclip` reference. The 80 ms lane matches `galion-heartbeat`
//! and `galion-ml-voice`. Presence stays the harness schema. CPU only.
#![forbid(unsafe_code)]

pub mod clip;
pub mod cpu;
pub mod error;
pub mod eval;
pub mod gq_write;
pub mod json;
pub mod load;
pub mod manifest;
pub mod persona;
pub mod pins;
pub mod safetensors;
pub mod sha;
pub mod translate;
pub mod wav;

pub use error::{Error, Result};
pub use eval::{run as self_eval, Report, Status};
pub use load::{open, Loaded};
pub use translate::{translate, TranslateOpts};

/// Heartbeat period. Same value as `galion_heartbeat::FRAME_MS`.
pub const CLOCK_MS: u32 = galion_heartbeat::FRAME_MS;
/// Live you/me lane. Same value as `galion_heartbeat::SAMPLE_RATE` and
/// `galion_ml_voice::stub::SAMPLE_RATE`. A reference clip may use another rate.
pub const LANE_HZ: u32 = galion_heartbeat::SAMPLE_RATE;
/// Samples in one 80 ms lane frame.
pub const FRAME_SAMPLES: u32 = galion_heartbeat::FRAME_SAMPLES as u32;
/// Presence report schema this pack names. The pack does not add a metric.
pub const PRESENCE_SCHEMA: &str = galion_presence::REPORT_SCHEMA;
pub const BANDS: &[&str] = &["you", "me", "feeling"];

const _: () = assert!(galion_ml_voice::stub::SAMPLE_RATE == galion_heartbeat::SAMPLE_RATE);
const _: () = assert!(galion_heartbeat::FRAME_MS == 80);
const _: () = assert!(galion_heartbeat::FRAME_SAMPLES == 1920);
const _: () = assert!(CLOCK_MS == 80 && LANE_HZ == 24_000 && FRAME_SAMPLES == 1920);
