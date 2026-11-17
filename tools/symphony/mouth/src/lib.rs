// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Mouth layer for Galion Voice.
//!
//! One 80 ms tick carries articulation targets for the cloner and a viseme
//! for any avatar. Feeling and intent set the direction. Magnitudes stay in
//! `configs/symphony/mouth.toml` until a source gives a number.
//!
//! Reading reuses the V1 card. SPARC, Phonet, Rhubarb and praatfan are not
//! reimplemented here.
//!
//! Text and the spoken pin come from `galion-mne-contract`.
//! Phones, smile, and precision are mouth-only extras the contract does not carry.
#![forbid(unsafe_code)]

pub mod coart;
pub mod direction;
pub mod error;
pub mod feeling;
pub mod intent;
pub mod inventory;
pub mod listen;
pub mod phone;
pub mod plan;
pub mod read;
pub mod scales;

/// Symphony heartbeat. LOCKS `clock.frame_ms` = 80.
pub const TICK_MS: u32 = 80;
pub const TICK_S: f64 = 0.08;

pub use error::MouthError;
pub use feeling::{parse_feelings, FeelingTick};
pub use intent::IntentStub;
pub use listen::{listen, MouthListen};
pub use plan::{plan, plan_value, MouthPlan};
pub use read::{read_json, read_value, read_via_binary, MouthRead};
pub use scales::{load_scales, parse_scales, Scales};
