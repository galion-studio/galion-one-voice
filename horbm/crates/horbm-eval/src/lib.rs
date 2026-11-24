// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Closed evaluation gate for the HORBM layers that are on main.
//!
//! A scripted turn is text plus, where the ear or the mouth needs a frame,
//! a generated 16 kHz tone. No voice recording and no dataset text.
//! Answers and citations come from the truth-root cards only.
//! `GALION_HORBM_RESEARCH_DIR` must be unset; the runner does not open it.
//!
//! On this path: card retrieval and [`horbm::Brain`], the emotion core,
//! [`galion_conductor::Conductor`], and [`horbm_voice::VoiceLoop`].
//! Vosk and Piper are not linked. [`horbm_voice::FakeEar`] and
//! [`horbm_voice::FakeMouth`] stand in behind [`horbm_voice::Ear`] and
//! [`horbm_voice::Mouth`]. Research stays the conductor's offline client.
//!
//! Time for the latency score is the virtual frame clock. The emotion
//! frame budget is the 80 ms lock, measured around [`horbm::Brain::step_moment`].

#![forbid(unsafe_code)]

mod gate;
mod score;
mod thresholds;

pub use gate::{run, run_with, Config};
pub use score::{
    score_emotion, score_grounding, score_latency, score_persona, score_unknown, EmotionSample,
    LatencySample, Spoken, VoiceTurn,
};
pub use thresholds::Thresholds;

/// Process environment. A non-empty value fails the gate. The path is not opened.
pub fn research_dir_from_env() -> Option<String> {
    std::env::var("GALION_HORBM_RESEARCH_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub fn research_dir_is_unset(value: Option<&str>) -> bool {
    value.map(|raw| raw.trim().is_empty()).unwrap_or(true)
}
