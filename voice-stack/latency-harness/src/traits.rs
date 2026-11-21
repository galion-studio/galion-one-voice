// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Stages the harness can time. Any implementation of these traits can be
//! passed to [`crate::measure`]. The CLI ships a fake set and a Piper mouth.
//!
//! HORBM is not in this repo. The real call, when that crate is linked, is:
//!
//! ```text
//! Brain::open(memory_db, knowledge_dir, cases)
//! brain.turn(&str) -> TurnOut
//! ```
//!
//! `TurnOut` here has the fields named in that interface: `reply`, `intent`,
//! `arm`, `propensity`, `evidence`, `sources`, `violations`, `lookup_us`,
//! `took_us`, and `mood: Pad`. `Pad::RESTING` is arousal 0.3, valence 0.25,
//! dominance 0.5. An adapter implements [`ReplyEngine`] and forwards
//! `turn`. This crate does not open a memory database and does not score cards.

use std::fmt;
use std::time::{Duration, Instant};

/// Fixed transcript the fake ASR returns after it has scanned real samples.
pub const HEARD: &str = "hello there";
/// Fixed reply the fake brain speaks. Short, plain, no emotion words.
/// Mood travels on [`TurnOut::mood`], not in this sentence.
pub const PHRASE: &str = "I am right here.";

/// Pleasure / arousal / dominance. Axes are the HORBM `Pad` fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    /// Resting mood from the HORBM interface.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };
}

/// One brain turn. Field names match the HORBM `TurnOut` the adapter will map.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
}

#[derive(Debug)]
pub struct StageError {
    pub stage: &'static str,
    pub message: String,
}

impl StageError {
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.stage, self.message)
    }
}

impl std::error::Error for StageError {}

/// Names stored on the report. Taken from each stage's `name()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageNames {
    pub vad: String,
    pub asr: String,
    pub reply: String,
    pub tts: String,
    pub audio_out: String,
}

/// End of user speech on a buffer that is already in memory.
///
/// The returned index is the first sample after the last speech frame.
/// The harness stamps end-of-speech when this call returns. That is an
/// offline decision, not a live microphone.
pub trait SpeechEnd {
    fn name(&self) -> &str;
    fn end_sample(&mut self, pcm: &[i16], sample_rate: u32) -> Result<usize, StageError>;
}

/// ASR final. The harness stamps the return.
pub trait AsrStage {
    fn name(&self) -> &str;
    fn finalize(&mut self, pcm: &[i16], sample_rate: u32) -> Result<String, StageError>;
}

/// Reply slot. `on_heartbeat` runs before `turn` with microseconds since
/// end of speech. The default ignores the tick. The fake slot uses it to
/// step mood. A real `horbm::Brain` ticks itself and can keep the default.
pub trait ReplyEngine {
    fn name(&self) -> &str;
    fn on_heartbeat(&mut self, _elapsed_us: u64) {}
    fn turn(&mut self, utterance: &str) -> Result<TurnOut, StageError>;
}

/// One chunk of PCM. `at` is when these samples became available to the
/// parent, stamped by the producer. The harness uses `at`, not the moment
/// it woke up.
#[derive(Clone, Debug)]
pub struct PcmChunk {
    pub at: Instant,
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}

/// Streaming TTS. `begin` starts one utterance. `pull` yields chunks until
/// `Ok(None)`. `finish` discards the rest so the next `begin` is clean.
/// The clock stops at the first heartbeat frame. `finish` is outside it.
pub trait TtsStage {
    fn name(&self) -> &str;
    fn begin(&mut self, text: &str, mood: &Pad) -> Result<(), StageError>;
    fn pull(&mut self, timeout: Duration) -> Result<Option<PcmChunk>, StageError>;
    fn finish(&mut self) -> Result<(), StageError>;
}

/// First playable frame. The buffer is one 24 kHz heartbeat frame
/// ([`crate::FRAME_SAMPLES`] `f32` samples in -1..1).
pub trait AudioOut {
    fn name(&self) -> &str;
    fn emit_frame(&mut self, frame: &[f32]) -> Result<(), StageError>;
}
