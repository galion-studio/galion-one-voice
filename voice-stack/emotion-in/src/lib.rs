// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Emotion in: the user's voice, on CPU, as a PAD mood target.
//!
//! Mic frames (80 ms, 24 kHz, the voice heartbeat) become pitch, energy,
//! speaking rate, and pauses. A fixed linear map turns those into a
//! [`Pad`] the HORBM brain can tick toward. The real `horbm` crate is not
//! linked. [`engine::FakeBrain`] stands in for `Brain::open` and `turn`.
//!
//! Raw audio is not kept. [`stream::EmotionIn`] is [`Copy`] and holds only
//! scalar rings. A framer may hold one unfinished frame and wipes it.

#![forbid(unsafe_code)]

pub mod clock;
pub mod consent;
pub mod engine;
pub mod error;
pub mod map;
pub mod pad;
pub mod pitch;
pub mod prosody;
pub mod report;
pub mod serve;
pub mod stream;
pub mod synth;
pub mod wav;

#[cfg(feature = "onnx")]
pub mod onnx;

pub use clock::{FRAME_MHZ, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE_HZ, STAGE_BUDGET_US};
pub use consent::{ConsentGate, Purpose, Session};
pub use engine::{FakeBrain, MoodTarget, ReplyEngine, TurnOut};
pub use error::{EmotionError, EmotionErrorKind};
pub use map::Classifier;
pub use pad::Pad;
pub use pitch::{Pitch, PitchCheck};
pub use prosody::Prosody;
pub use report::{dot_view, report_json, stage_text, DotView, Report, StageTimes};
pub use serve::{bind_loopback, handle_client, Publish};
pub use stream::{analyse_pcm, measure_stages, synthetic_scenarios, EmotionIn, Reading, Scenario};
pub use wav::{analyse_pcm_s16, analyse_wav_path, analyse_wav_reader, PcmS16, WavPull};
