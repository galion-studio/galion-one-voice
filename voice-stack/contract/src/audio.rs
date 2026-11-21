// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Audio, VAD, and playback signatures.
//!
//! [`AudioIn`] matches `galion_console::mic::MicSource` (`fill`, `sample_rate`,
//! `name`). S01 implements that trait when it already depends on
//! `galion-console`, and this trait otherwise. This crate does not depend on
//! `galion-console` (that crate pulls the terminal UI).

use crate::ProsodyControl;

/// One mono PCM16 buffer. `samples` is audio the pipeline already holds.
/// This crate's tests use a handful of integers, not a recording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmFrame {
    pub seq: u64,
    pub t_ms: u32,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioError {
    Closed,
    Rate(u32),
    Backend(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::Closed => f.write_str("audio closed"),
            AudioError::Rate(r) => write!(f, "unexpected sample rate {r}"),
            AudioError::Backend(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for AudioError {}

pub trait AudioIn {
    fn fill(&mut self, tick: u64, out: &mut [f32]);
    fn sample_rate(&self) -> u32;
    fn name(&self) -> &str;
}

pub trait AudioOut {
    fn write_frame(&mut self, frame: &PcmFrame) -> Result<(), AudioError>;
    /// Barge-in. Drop what has not been played. Do not apologise in audio.
    fn stop(&mut self);
    fn sample_rate(&self) -> u32;
}

/// Same names as `galion_v2_live::endpoint::EpEv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointEvent {
    None,
    Start,
    Speculate,
    Resume,
    Commit,
    Hold,
}

/// One 20 ms frame plus the partial transcript, as `Endpointer::push` takes.
pub trait VadStage {
    fn push(&mut self, frame: &[i16], frame_ms: f64, partial: &str) -> EndpointEvent;
}

/// S07. Caller affect is absent unless `sense` is granted. The brain `Pad`
/// is her own mood and does not require `sense`.
pub trait ProsodyMap {
    fn map(&mut self, mood: crate::Pad) -> ProsodyControl;

    fn map_with_caller(
        &mut self,
        mood: crate::Pad,
        caller: Option<&crate::EmotionFrame>,
        sense: bool,
    ) -> ProsodyControl {
        // The default does not invent a caller-to-prosody map. S07 overrides
        // this once it has one, and still drops `caller` when `sense` is false.
        let _ = (caller, sense);
        self.map(mood)
    }
}

/// Inbound affect without `sense` is absence, not a neutral frame.
pub fn consented_frame(
    sense: bool,
    frame: Option<crate::EmotionFrame>,
) -> Option<crate::EmotionFrame> {
    if sense {
        frame
    } else {
        None
    }
}

/// S08. `None` means no consented reading, not a neutral guess.
pub trait EmotionIn {
    fn estimate(&mut self, pcm: &[f32], t_ms: u32, sense: bool) -> Option<crate::EmotionFrame>;
}
