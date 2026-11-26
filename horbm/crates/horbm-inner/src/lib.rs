// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Inner life for whatever persona the host configured.
//!
//! This is the slow layer. It is not the steady voice baseline and it is
//! not the 80 ms moment. Care, curiosity, wanting to help, and expecting an
//! answer move on a turn and decay on a tick. The note they send is
//! [`InnerMsg`]. The emotion core already speaks [`galion_emotion_core::IntentMsg`]
//! (`GEMI`, 72 bytes). [`InnerMsg::to_gemi`] is that record, so the moment
//! can step it. A step does not write the baseline.
//!
//! The older token layer in `galion-emotion-core` (`InnerLife`, surprise,
//! `[consent] learn`) stays where it is. This crate does not replace it.
//! Both speak the same GEMI record.
//!
//! The home values come from the host as plain numbers. Build an [`InnerHome`]
//! with [`InnerHome::from_baseline`] from the configured persona's baseline
//! (`Persona::load_development` / `BootConfig` in `horbm-persona`) and pass
//! that persona's id to [`PersonaInner::with_home_for`]. This crate does not
//! read a persona file and does not name one. Nothing about the person is
//! stored. `[consent] remember` stays deny. State is the struct the caller
//! holds for the session.
//!
//! [`PersonaInner::new`] is the standalone home: the same four numbers this
//! layer used before a host could pass its own, and an empty persona id.
//!
//! # Message
//!
//! Serde object, one level, no transcript field:
//!
//! ```text
//! dir:        "out" | "in"
//! verb:       "comfort" | "ask" | "wait" | "with" | "answer" | "check-in" | "hold" | "heard"
//! strength:   f32  0..=1     (inbound: confidence of the reading)
//! t_ms:       u32
//! care:       f32  0..=1
//! curiosity:  f32  0..=1
//! help:       f32  0..=1
//! expect:     f32  0..=1
//! valence:    f32            (out: pull; in: listener pleasure)
//! arousal:    f32            (out: pull; in: listener activation)
//! dominance:  f32            (out: pull)
//! approach:   f32            (out: pull)
//! sense:      bool           (ear switch; off ignores a listener point)
//! ```
//!
//! Outbound GEMI uses source own, factor mask 0, and consent equal to the
//! sense bit or zero. Inbound `heard` carries the listener point in the
//! pull slots. Those slots are not applied to the voice body by this crate.

#![forbid(unsafe_code)]

mod life;
mod msg;

pub use life::{DialogueLean, Heard, InnerHome, Perceived, PersonaInner};
pub use msg::{InnerMsg, MsgDir, Verb};

/// `[consent] remember`. Not written, and not a fifth wire bit.
pub const REMEMBER_LOCK: &str = "deny";

pub fn remember_denied() -> bool {
    REMEMBER_LOCK == "deny"
}

/// Tick quantum. Same 80 ms as the moment core and `LOCKS.toml` `[clock]`.
pub const TICK_MS: u32 = galion_emotion_core::FRAME_MS;

/// Bytes in the persona id slot. ASCII only, copied by the host, no heap.
pub const PERSONA_ID_BYTES: usize = 16;

/// Added to a baseline valence to place home care, then clamped to `0..=1`.
pub const CARE_ABOVE_REST: f32 = 0.30;

/// Deprecated: use PersonaInner / InnerHome.
///
/// Standalone home care: [`CARE_ABOVE_REST`] above a rest valence of 0.25.
pub const CARE_HOME: f32 = CARE_ABOVE_REST + 0.25;

/// Deprecated: use PersonaInner / InnerHome.
///
/// Standalone home curiosity.
pub const CURIOSITY_HOME: f32 = 0.42;

/// Deprecated: use PersonaInner / InnerHome.
///
/// Standalone home for wanting to help.
pub const HELP_HOME: f32 = 0.36;

/// Deprecated: use PersonaInner / InnerHome.
///
/// Standalone home for expecting an answer. Quiet until a question.
pub const EXPECT_HOME: f32 = 0.18;

/// Where care goes on a sad turn. One step closes half the gap.
pub const CARE_SAD: f32 = 0.92;
pub const HELP_SAD: f32 = 0.74;
pub const EXPECT_QUESTION: f32 = 0.90;
pub const HELP_QUESTION: f32 = 0.62;
pub const CURIOSITY_ASK: f32 = 0.84;

/// Fraction of the gap a turn closes.
pub const TURN_STEP: f32 = 0.50;

/// Per tick, keep this much of the gap to home.
/// 997/1000 is the discrete form of a 20 s mood half-life at 80 ms.
pub const DECAY_KEEP_NUM: u32 = 997;
pub const DECAY_KEEP_DEN: u32 = 1000;

/// Quiet before the intent is `wait`. Company, without a new topic.
pub const WAIT_MS: u32 = 4_000;
/// Quiet before one gentle `check-in`.
pub const CHECKIN_MS: u32 = 12_000;
/// Minimum gap between check-ins. One mood half-life, used as a rate limit.
pub const CHECKIN_GAP_MS: u32 = 20_000;

/// Word valence at or below this, from `text_affect`, counts as sad.
/// "tired" (−0.4) does not. "sad" (−0.7) does.
pub const SAD_WORD_VALENCE: f32 = -0.45;
/// Listener pleasure at or below this counts as sad when the ear is on.
pub const SAD_EAR_VALENCE: f32 = -0.20;
/// A reading under this confidence is not a claim.
pub const EAR_CONFIDENCE: f32 = 0.35;
