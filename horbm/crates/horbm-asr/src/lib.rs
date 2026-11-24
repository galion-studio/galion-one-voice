// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Speech recognition for the HORBM voice stack.
//!
//! One call accepts one ear frame: 80 ms of 16 kHz mono PCM, 1280 samples.
//! That is the native ear's frame, not the 24 kHz mouth frame. Partials and
//! finals carry timestamps on that clock. [`SpeechRecognizer::cancel`] drops
//! the open utterance for barge-in and does not emit a final.
//!
//! [`StandIn`] is the closed-test engine. It does not read the samples and
//! it does not load weights. The Vosk engine is `--features vosk` and is
//! off by default. This crate does not probe a GPU and does not call into
//! a GPU init. Recognition stays on the CPU.
//!
//! The host runs this beside `galion-conductor`'s `Conductor::tick`, not
//! inside the heartbeat frame step. Page: `docs/horbm/s22-speech.md`.
//!
//! # Safety
//!
//! No `unsafe`. No socket. No model weights in the crate.

#![forbid(unsafe_code)]

#[cfg(feature = "vosk")]
mod engine;
mod standin;

#[cfg(feature = "vosk")]
pub use engine::{open_vosk, open_vosk_from_env, EngineSpeech};
#[cfg(feature = "vosk")]
pub use galion_sam_asr::VoskConfig;
pub use standin::StandIn;

/// Locked frame. `LOCKS.toml` `[clock] frame_ms`. Same quantum as
/// `galion-conductor` and the native ear.
pub const FRAME_MS: u32 = 80;

/// Ear rate. The mouth and the heartbeat frame are 24 kHz. This port is not.
pub const SAMPLE_RATE: u32 = 16_000;

/// Samples in one [`FRAME_MS`] frame at [`SAMPLE_RATE`].
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;

/// Barge-in lock. `LOCKS.toml` `[gates] barge_in_max_ms`. Inclusive.
pub const BARGE_IN_MAX_MS: u32 = 200;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(SAMPLE_RATE == 16_000);
const _: () = assert!(FRAME_SAMPLES == 1_280);
const _: () = assert!(BARGE_IN_MAX_MS == 200);
const _: () = assert!(FRAME_SAMPLES * 1000 / (SAMPLE_RATE as usize) == FRAME_MS as usize);

/// Where recognition runs. CPU only. A later backend can add a variant.
/// A GPU on the machine does not select one: the voice stays on the CPU.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputePlace {
    Cpu,
}

/// This build recognizes on the CPU. It does not stat a device node.
pub fn place() -> ComputePlace {
    ComputePlace::Cpu
}

/// `audio_ms` is still inside the barge-in lock.
///
/// Zero is outside. The lock is inclusive. Cancel itself is not refused
/// when the window has passed; the host uses this to decide.
pub fn within_barge_window(audio_ms: u32) -> bool {
    audio_ms > 0 && audio_ms <= BARGE_IN_MAX_MS
}

/// Partial hypotheses can be replaced. The final is the text to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptKind {
    Partial,
    Final,
}

/// One word and its span on the utterance clock, in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedWord {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// One hypothesis. `start_ms` and `end_ms` are on the utterance clock.
///
/// `end_ms` is the audio accepted for this hypothesis. A word's own `end_ms`
/// can be earlier when trailing silence was pushed after the last word.
/// Empty text is not a transcript. A failure is [`RecogError`], not a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub kind: TranscriptKind,
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub words: Vec<TimedWord>,
}

/// Streaming recognizer. One thread. No async runtime.
///
/// [`SpeechRecognizer::push_frame`] takes one ear frame. [`SpeechRecognizer::finish`]
/// closes the utterance. [`SpeechRecognizer::cancel`] is barge-in: the open
/// hypothesis is dropped and no final is emitted.
pub trait SpeechRecognizer {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Transcript>, RecogError>;
    fn finish(&mut self) -> Result<Vec<Transcript>, RecogError>;
    fn cancel(&mut self) -> Result<(), RecogError>;
    /// Audio accepted in the open utterance, in milliseconds.
    fn audio_ms(&self) -> u32;
    fn name(&self) -> &str;
}

/// A frame was the wrong length, or the engine failed.
///
/// The display text is not a transcript. Callers do not speak it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecogError {
    Frame { got: usize, expected: usize },
    Engine(String),
}

#[cfg(feature = "vosk")]
impl RecogError {
    pub(crate) fn engine(err: impl std::fmt::Display) -> Self {
        Self::Engine(err.to_string())
    }
}

impl std::fmt::Display for RecogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame { got, expected } => {
                write!(f, "audio frame is {got} samples, want {expected}")
            }
            Self::Engine(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for RecogError {}

pub(crate) fn check_frame(pcm: &[i16]) -> Result<(), RecogError> {
    if pcm.len() == FRAME_SAMPLES {
        Ok(())
    } else {
        Err(RecogError::Frame {
            got: pcm.len(),
            expected: FRAME_SAMPLES,
        })
    }
}
