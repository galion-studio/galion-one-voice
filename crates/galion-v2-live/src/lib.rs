// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2 live lane. Every engine runs out of process (Piper, Vosk sidecar,
//! clone worker, llama-server or curl for the LLM). Nothing here is a stub: a missing
//! engine is an error, never a fake clock.
pub mod audio;
pub mod piper;
pub mod asr;
pub mod llm;
pub mod vad;
pub mod clone;
pub mod stats;
pub mod endpoint;
pub mod warm;
pub mod local;

use std::time::Instant;
use std::sync::OnceLock;

static T0: OnceLock<Instant> = OnceLock::new();
/// Milliseconds since process start (monotonic), f64.
pub fn now_ms() -> f64 {
    T0.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}
pub fn repo_root() -> std::path::PathBuf {
    std::env::var("GALION_REPO").map(Into::into).unwrap_or_else(|_| std::env::current_dir().unwrap())
}
