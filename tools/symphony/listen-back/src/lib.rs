// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Listening back (swarm 13, layer S7).
//!
//! For each fixture: take the intent, render, decode with the articulation
//! analyser (the same praatfan path PR #191 wraps), compare the read-back to
//! the fixture, score the layers, apply one correction, and score again.
//!
//! This crate does not reimplement the decoder, the lungs planner, or the
//! emotion-to-controls map. Those stay on their drafts. Missing layers are
//! named stubs.
#![forbid(unsafe_code)]

pub mod contract;
pub mod decode;
pub mod fixture;
pub mod render;
pub mod report;
pub mod run;
pub mod score;

pub use report::{summary, Report, RolloutGate};
pub use run::{run_all, Options};

/// Existing rollout gate. Recorded, not measured, and not applied here.
pub const ROLLOUT_WER_MAX: f64 = 0.018;
pub const ROLLOUT_ECAPA_MIN: f64 = 0.7057;
pub const ROLLOUT_EMOTION_MIN: f64 = 0.83;

pub fn rollout_gate() -> RolloutGate {
    RolloutGate {
        wer_max: ROLLOUT_WER_MAX,
        ecapa_min: ROLLOUT_ECAPA_MIN,
        emotion_min: ROLLOUT_EMOTION_MIN,
        measured: false,
        note: "Existing rollout gate. Recorded and not measured. This harness is not a gate. WER <= 1.8%, ECAPA > 0.7057, emotion >= 0.83.".into(),
    }
}

pub(crate) fn round4(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    (x * 10_000.0).round() / 10_000.0
}
