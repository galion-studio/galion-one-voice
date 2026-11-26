// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Closed full-duplex loop for HORBM.
//!
//! One call to [`Duplex::tick`] is one 80 ms frame at 16 kHz. The you band
//! and the me band are both stepped on that call. The configured persona can
//! be speaking while a new frame is still recognized. A listed backchannel
//! does not cancel that turn. Any other overlap stops the me band on the frame that starts
//! at 160 ms, inside the 200 ms lock.
//!
//! The brain is [`horbm::Brain`]. The floor is [`galion_conductor::Conductor`].
//! The mouth on the live path is [`NativeMouth`] unless the crate is built
//! with `--features piper` and the host asks for [`PiperMouth`]. Emotion is
//! [`CoreAffect`] (`galion-emotion-core`). Each frame with a moment is handed
//! to the mouth. Prosody sensing is on unless [`Duplex::set_prosody`] turns
//! it off. Echo cancellation is [`PlaybackEcho`] over `fdaf-aec` unless
//! [`Duplex::set_aec`] bypasses it, or the crate is built with
//! `--no-default-features`. The recognizer is [`SilentRecognizer`] unless
//! the crate is built with `--features vosk`. Breath is still a stand-in.
//! The virtual-clock integration test keeps its own stand-ins and is the CI gate.
//!
//! This tick is not the heartbeat frame step. It does not read a wall clock,
//! open a socket, or load a weight.
//!
//! # Safety
//!
//! No `unsafe`.

#![forbid(unsafe_code)]

mod brain;
mod breath;
mod duplex;
mod echo;
mod emotion;
mod live;
mod mouth;
mod persona;
#[cfg(feature = "piper")]
mod piper_mouth;
mod ports;
mod prosody;
mod recog;
mod report;

pub use breath::SpeakingBreath;
pub use duplex::{Duplex, Floor, FrameOut, LoopEvent, Parts};
pub use echo::{PlaybackEcho, AEC_CRATE, AEC_LICENSE, AEC_VERSION};
pub use emotion::CoreAffect;
pub use horbm_voice::{FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};
pub use live::FramePump;
pub use mouth::NativeMouth;
pub use persona::{open_persona, ConfiguredPersona};
#[cfg(feature = "piper")]
pub use piper_mouth::{
    discover_piper, measure_piper, piper_cache_dir, piper_skipped_json, PiperMouth, PiperPaths,
    PiperStats, STOCK_VOICE,
};
pub use ports::{
    Affect, Breath, BreathCue, BreathPhase, EarSense, Emotion, Hypothesis, HypothesisKind, Persona,
    ProsodyCue, RecogError, Recognizer, Style, BREVITY_RATE,
};
pub use prosody::ProsodyEar;
pub use recog::SilentRecognizer;
pub use report::{Budgets, LatencyReport};

#[cfg(feature = "vosk")]
pub use recog::AsrRecognizer;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(FRAME_MS == horbm::FRAME_MS);
const _: () = assert!(FRAME_MS == galion_conductor::FRAME_MS);
const _: () = assert!(SAMPLE_RATE == 16_000);
const _: () = assert!(FRAME_SAMPLES == 1_280);
const _: () = assert!(galion_conductor::BARGE_IN_MAX_MS == 200);
const _: () = assert!(galion_conductor::HANG_FRAMES == 1);
const _: () = assert!(Budgets::locked().affect_frames * FRAME_MS == Budgets::locked().affect_ms);
const _: () = assert!(Budgets::locked().barge_frames * FRAME_MS == Budgets::locked().barge_ms);
const _: () = assert!(Budgets::locked().barge_ms <= galion_conductor::BARGE_IN_MAX_MS);
const _: () = assert!(Budgets::locked().voice_p50_ms <= Budgets::locked().voice_p95_ms);
const _: () = assert!(Budgets::locked().voice_p95_ms <= Budgets::locked().onset_ms);

/// Why a frame or a persona was refused. The clock does not move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopError {
    Frame { got: usize, expected: usize },
    Recognizer(String),
    Persona(&'static str),
}

impl std::fmt::Display for LoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame { got, expected } => {
                write!(f, "audio frame is {got} samples, want {expected}")
            }
            Self::Recognizer(message) => f.write_str(message),
            Self::Persona(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for LoopError {}

impl From<RecogError> for LoopError {
    fn from(err: RecogError) -> Self {
        match err {
            RecogError::Frame { got, expected } => Self::Frame { got, expected },
            RecogError::Engine(message) => Self::Recognizer(message),
        }
    }
}
