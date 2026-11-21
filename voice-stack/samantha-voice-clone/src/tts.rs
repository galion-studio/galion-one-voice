// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Shared TTS contract for the voice stack.
//! `FakeTts` is the stand-in other sectors can use until a real backend is wired.

use crate::error::TtsError;
use crate::frames::{frames_of, tail_of, FRAME_SAMPLES, SAMPLE_RATE_HZ};
use crate::recipe::SentenceJob;

/// Live mood. The s711 knobs stay at the approved recipe.
/// Emotion is not a scripted line in this crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    pub fn is_finite(self) -> bool {
        self.arousal.is_finite() && self.valence.is_finite() && self.dominance.is_finite()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spoken {
    pub samples: Vec<i16>,
    pub device: String,
    /// Microseconds the worker reported for this sentence, when it reported one.
    pub worker_ttfa_us: Option<u128>,
    pub from_cache: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SentenceAudio {
    pub index: usize,
    pub seed: u32,
    pub text: String,
    pub samples: Vec<i16>,
    pub cached: bool,
    pub worker_ttfa_us: Option<u128>,
    pub reference_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Speech {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits: u16,
    pub samples: Vec<i16>,
    pub sentences: Vec<SentenceAudio>,
    /// True only when every sentence came from the phrase cache.
    pub cached: bool,
    pub device: String,
    /// Wall time of this call, including cache hits. Measured with `Instant`.
    pub took_us: u128,
    pub recipe: &'static str,
    pub mood: Pad,
}

impl Speech {
    pub fn full_frames(&self) -> impl Iterator<Item = &[i16]> {
        frames_of(&self.samples)
    }

    pub fn tail(&self) -> &[i16] {
        tail_of(&self.samples)
    }

    pub fn frame_sample_count(&self) -> usize {
        FRAME_SAMPLES
    }
}

pub trait Tts {
    fn synthesize(&mut self, text: &str, mood: &Pad) -> Result<Speech, TtsError>;

    fn sample_rate_hz(&self) -> u32 {
        SAMPLE_RATE_HZ
    }
}

pub trait SentenceSpeaker {
    fn speak(&mut self, job: &SentenceJob) -> Result<Spoken, TtsError>;
}

/// In-memory speaker for tests and for a pipeline that does not have the model.
#[derive(Clone, Debug, Default)]
pub struct RecordingSpeaker {
    pub jobs: Vec<SentenceJob>,
}

impl SentenceSpeaker for RecordingSpeaker {
    fn speak(&mut self, job: &SentenceJob) -> Result<Spoken, TtsError> {
        self.jobs.push(job.clone());
        let mut samples = vec![0i16; FRAME_SAMPLES];
        samples[0] = i16::try_from(job.seed).unwrap_or(i16::MAX);
        Ok(Spoken {
            samples,
            device: "recording".into(),
            worker_ttfa_us: None,
            from_cache: false,
        })
    }
}

/// Public fake `Tts`. One silent 80 ms frame. No file, no worker.
#[derive(Clone, Debug, Default)]
pub struct FakeTts {
    pub calls: Vec<String>,
}

impl Tts for FakeTts {
    fn synthesize(&mut self, text: &str, mood: &Pad) -> Result<Speech, TtsError> {
        if !mood.is_finite() {
            return Err(TtsError::new("mood", "mood values must be finite"));
        }
        if text.trim().is_empty() {
            return Err(TtsError::new("text", "empty text"));
        }
        self.calls.push(text.to_string());
        Ok(Speech {
            sample_rate_hz: SAMPLE_RATE_HZ,
            channels: 1,
            bits: 16,
            samples: vec![0; FRAME_SAMPLES],
            sentences: Vec::new(),
            cached: false,
            device: "fake".into(),
            took_us: 0,
            recipe: "fake",
            mood: *mood,
        })
    }
}
