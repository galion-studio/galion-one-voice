// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Streaming CPU speech-to-text for the Samantha voice stack (swarm 16, sector S03).
//!
//! Audio in is 16 kHz mono PCM16. [`Asr::push_pcm`] returns partial snapshots.
//! [`Asr::finish`] closes the utterance. The text to keep is the final, not the
//! partials glued on after it.
//!
//! The Vosk engine follows `galion-asr-vosk` (PR #26): same small-en model,
//! same libvosk symbols, 20 ms frames, `[unk]` dropped, no GPU init. The
//! optional `whisper` feature builds whisper.cpp through whisper-rs and stays
//! out of the default build.

#[allow(dead_code)]
mod b64;
mod engine;
mod error;
mod event;
mod fake;
mod json;
mod manifest;
mod open;
mod paths;
mod score;
mod stream;
mod text;
mod timefmt;
mod vosk;
mod vosk_ffi;
mod wav;
mod wer;
mod worker;

#[cfg(feature = "whisper")]
mod whisper;

pub use engine::Asr;
pub use error::AsrError;
pub use event::{
    samples_to_ms, AsrEvent, AsrKind, Word, HEARTBEAT_MS, HEARTBEAT_SAMPLES, SAMPLE_RATE,
    VOSK_FRAME_SAMPLES,
};
pub use fake::FakeAsr;
pub use manifest::{parse_manifest, ClipRef};
pub use open::{open_engine, EngineOpts};
pub use paths::{cpu_model, find_vosk_lib, find_vosk_model, find_whisper_model};
pub use score::{percentile_us, ClipScore, HostInfo, RunScore};
pub use stream::{final_text, run_stream, StreamReport};
pub use text::clean_transcript;
pub use timefmt::unix_to_utc;
pub use vosk::{VoskAsr, VoskConfig};
pub use wav::{read_wav, read_wav_bytes, wav_to_16k_mono, PcmWav};
pub use wer::{corpus_counts, normalize_words, word_error, Wer};
pub use worker::VoskWorkerAsr;

#[cfg(feature = "whisper")]
pub use whisper::{WhisperAsr, WhisperConfig};

/// Compiler that built this crate, captured by `build.rs`.
pub fn compiled_with() -> &'static str {
    env!("S03_RUSTC")
}
