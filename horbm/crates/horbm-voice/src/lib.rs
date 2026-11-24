// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM-native voice turn.
//!
//! One tick is one conductor frame ([`clock::FRAME_MS`] = 80). The host calls
//! [`VoiceLoop::tick`] on the turn worker, beside the heartbeat frame step.
//! This function is not that step. The clock moves only when `tick` runs.
//!
//! Order, fixed: ear, then [`horbm::Brain::turn`], then mouth. Piper and Vosk
//! are feature-gated adapters. The default build links neither.
//!
//! # Safety
//!
//! No `unsafe`. No wall clock. No socket.

#![forbid(unsafe_code)]

mod brain;
mod clock;
mod device;
mod ear;
mod lexicon;
mod mouth;
mod turn;

#[cfg(feature = "piper")]
mod piper_mouth;
#[cfg(feature = "vosk")]
mod vosk_ear;

pub use brain::{HorbmTurn, SpokenLine, TurnBrain};
pub use clock::{
    VirtualClock, BARGE_IN_MAX_MS, FIRST_AUDIO_BUDGET_MS, FRAME_MS, FRAME_SAMPLES, PAUSE_MS,
    SAMPLE_RATE,
};
pub use device::{place, select_place, Place};
pub use ear::{Ear, EarEvent, FakeEar};
pub use horbm::Pad;
pub use lexicon::is_listed_backchannel;
pub use mouth::{AudioFrame, FakeMouth, Mouth, MouthPull};
pub use turn::{TickEvent, TickOut, TurnMsg, VoiceLoop};

#[cfg(feature = "piper")]
pub use piper_mouth::PiperMouth;
#[cfg(feature = "vosk")]
pub use vosk_ear::VoskEar;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(FRAME_MS == horbm::FRAME_MS);
const _: () = assert!(BARGE_IN_MAX_MS == 200);
const _: () = assert!(FIRST_AUDIO_BUDGET_MS <= 240);
const _: () = assert!(FIRST_AUDIO_BUDGET_MS >= FRAME_MS);
