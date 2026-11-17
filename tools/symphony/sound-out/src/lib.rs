// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Sound out. The last stage before the listener.
//!
//! One [`ClonerBackend`] trait. Piper is the realtime CPU default. Chatterbox
//! is an optional offline backend. Moshi is GPU-only and opt-in. Speech is placed on
//! the lungs timeline from PR #195, bursts from the nose layer (S5) are
//! added, and the moment's room, distance and noise are applied.
//!
//! The intent contract (`galion-mne-contract`, agent S1) is not in this
//! tree. [`contract`] is a local stand-in.
#![forbid(unsafe_code)]

pub mod backend;
pub mod chatterbox;
pub mod contract;
pub mod device;
pub mod environment;
pub mod moshi;
pub mod piper;
pub mod policy;
pub mod request;
pub mod session;

pub use backend::{
    Capabilities, ClonerBackend, PcmChunk, SpeakMeta, SAMPLES_PER_FRAME, SAMPLE_RATE,
};
pub use chatterbox::ChatterboxBackend;
pub use contract::MomentFixture;
pub use device::{choose, detect, detect_with, DeviceChoice, DeviceKind};
pub use environment::{Burst, Environment, Room};
pub use moshi::MoshiStub;
pub use piper::PiperBackend;
pub use policy::Policy;
pub use request::CloneRequest;
pub use session::{render, Session, SoundOut};

/// 80 ms Symphony clock. Same constant as `galion-lungs`.
pub const TICK_MS: u32 = galion_lungs::TICK_MS;
