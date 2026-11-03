// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Voice-side seam to Obsidian Open Mind.
//!
//! Two channels, neither of them inside `Heartbeat::step` (this crate does not
//! depend on `galion-heartbeat`; the rule is the 80 ms lock):
//!
//! * [`nerve`] observes the voice engine and emits three `NerveEvent` kinds
//!   (`voice.frame_miss`, `voice.fallback`, `voice.backend_up`). The caller
//!   only enqueues a [`VoiceNote`] (copy, non-blocking). A side thread formats
//!   NDJSON. With no nerve daemon the lines stay in a RAM ring, and optionally
//!   a local file. A loopback `POST /event` is attempted only when a human sets
//!   `GALION_NERVE_URL` to a numeric loopback address.
//! * [`handoff`] is the cortex. It submits [`galion_sideband::ToHermes`]
//!   `Turn` / `Intent` / `Affect` through the existing side-band client.
//!   Hermes thinks off the audio tick. The 3 s retrieval cap still applies.
//!
//! Obsidian Open Mind itself is not vendored. It lives at
//! <https://github.com/galion-studio/obsidian-open-mind> and, when a human
//! has deployed it, under `/opt/galion/openmind` on the VPS. This crate does
//! not clone that tree, does not open SSH, and does not download a model.
//!
//! Design note: `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`.
#![forbid(unsafe_code)]

pub mod handoff;
pub mod nerve;
pub mod observe;

pub use handoff::{CortexHandoff, HandoffReceipt, IntentDisposition, TakeoverOffer};
pub use nerve::{
    parse_loopback_http, render_event, NerveConfig, NerveEmitter, NerveGroup, NerveSink,
    NerveStats, NoteDrop, RenderedEvent, Severity, VoiceBackendSlot, VoiceKind, VoiceNote,
    NERVE_SCHEMA_VERSION, SOURCE, VOICE_KINDS,
};
pub use observe::note_from_frame;
