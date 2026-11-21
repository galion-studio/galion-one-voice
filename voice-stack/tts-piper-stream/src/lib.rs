// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Streaming CPU Piper fallback for the Samantha voice stack (sector S05).
//!
//! One warm `piper --json-input --output_raw` process. Each clause is its own
//! request. The next clause is submitted before the caller is handed the
//! previous clause's PCM, so playback of clause 1 overlaps synthesis of
//! clause 2. Piper VITS still emits a whole clause at once.
//!
//! The [`Tts`] trait is the mouth contract shared with the Samantha clone
//! backend (S06). [`PiperTts`] is the stock-voice fallback. [`FakeTts`] is the
//! in-process stand-in for tests. This crate does not open `GALION_SAMANTHA_REF`
//! and does not load a reference clip.
#![deny(unsafe_code)]

mod clause;
mod frames;
mod mood;
mod piper;
mod protocol;
mod session;
mod tts;

pub use clause::{prepare_text, split_clauses, ClauseBuf, CLAUSE_WORDS};
pub use frames::{
    frames_80ms, resample_linear, Frame80, HEARTBEAT_FRAME, HEARTBEAT_HZ, HEARTBEAT_MS,
};
pub use mood::{shape_pcm, Prosody};
pub use piper::{
    resolve, verify_voice, IoMode, PiperTts, PiperVoice, PINNED_JSON_SHA, PINNED_ONNX_SHA,
    VOICE_JSON, VOICE_ONNX,
};

/// Milliseconds the thread IO path waits to confirm the pipe is drained.
/// The Linux poll path does not wait. Time-to-first-byte does not include this.
pub fn thread_drain_confirm_ms() -> u64 {
    session::THREAD_DRAIN_CONFIRM_MS
}
pub use protocol::{json_line, parse_rtf, Rtf};
pub use tts::{
    stand_in_pcm, FakeTts, Pad, PcmChunk, SentenceStat, SpeakReport, StreamEvent, Tts, TtsError,
};
