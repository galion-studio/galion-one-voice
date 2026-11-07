// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Controlled-reference checker for Emotion V2 capture (432 Hz standard).
//!
//! The procedure is `docs/emotion-v2/CAPTURE-PROTOCOL.md`. The numbers are
//! `docs/emotion-v2/capture-spec.toml`. This crate measures a WAV. It does not
//! synthesize a voice and it does not clone one.

#![forbid(unsafe_code)]

pub mod check;
pub mod dsp;
pub mod manifest;
pub mod spec;
pub mod synth;
pub mod wav;

pub use check::{check_file, check_pcm, check_wav_bytes, Report, Role};
pub use spec::{load_spec, Spec, EMOTIONS};
pub use wav::{encode, parse_wav, read_wav, Pcm};
