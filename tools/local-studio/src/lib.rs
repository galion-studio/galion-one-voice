// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Local studio pipeline for Galion One.
//!
//! A reference clip is a path the operator passes at runtime. This crate
//! does not ship a voice, does not download weights, and does not link an
//! inference runtime. Zero-shot rendering, whisper, ECAPA, the emotion
//! classifier, and ffmpeg are existing programs on the machine.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bed;
pub mod command;
pub mod config;
pub mod device;
pub mod doctor;
pub mod master;
pub mod pipeline;
pub mod render;
pub mod score;
pub mod script;
pub mod wer;

/// Rollout intelligibility gate. Word error rate at or below this passes.
pub const ROLLOUT_WER_MAX: f64 = 0.018;
/// Rollout speaker-similarity gate. The score must be greater than this.
pub const ROLLOUT_ECAPA_MIN: f64 = 0.7057;
/// Rollout emotion gate. The directed-emotion score must be at least this.
pub const ROLLOUT_EMOTION_MIN: f64 = 0.83;
/// Delivery integrated loudness.
pub const TARGET_LUFS: f64 = -16.0;
/// Delivery true-peak ceiling.
pub const TRUE_PEAK_DBTP: f64 = -1.0;
/// Music bed tuning. Generated in process. No sample library.
pub const BED_HZ: f64 = 432.0;
/// Bed root-mean-square level under the voice, in decibels.
pub const BED_UNDER_DB: f64 = 18.0;
/// Tracked wav, mp3, opus, and flac files above this size fail CI.
pub const TINY_FIXTURE_BYTES: u64 = 4096;
