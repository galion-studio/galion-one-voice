// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Per-stage latency harness for the local Samantha voice path.
//!
//! The clocks match the in-tree 80 ms heartbeat and the Piper stream TTFA
//! drain. This crate does not link those crates. It is its own workspace so
//! the root `Cargo.toml` members list stays unchanged.
//!
//! Stages, timed from the end of user speech:
//! end of speech, ASR final, reply ready, first TTS audio, first audio out.
//!
//! [`ReplyEngine`] is the slot where `horbm::Brain` will land. This crate
//! does not reimplement that brain. [`fake::FakeReply`] fills the slot in
//! tests and in the published runs until the real crate is on the machine.
//!
//! No audio is written. A missing Piper pack is `measured: false` with no
//! percentile fields.

#![forbid(unsafe_code)]

pub mod clock;
pub mod fake;
pub mod machine;
pub mod pipeline;
pub mod piper;
pub mod report;
pub mod sha256;
pub mod stats;
pub mod traits;

pub use clock::{
    advance_toward_rest, heartbeat_play_us, input_samples_for_one_frame, resample_to_frame,
    schedule_frames, BAR_US, FRAME_HZ_TENTHS, FRAME_MS, FRAME_SAMPLES, PIPER_HZ, SAMPLE_RATE_HZ,
    STEP_BUDGET_US,
};
pub use fake::{fixture_pcm, run_fake, EnergyVad, FakeAsr, FakeReply, FakeTts, FrameSink};
pub use machine::{format_unix_utc, probe_machine, redact_host, repo_root, Machine};
pub use pipeline::{measure, HarnessError, MeasureIn};
pub use piper::{locate_piper, run_piper, PiperInfo, PiperPaths};
pub use report::{ms_text, ClockKind, Rep, Report, StageStat};
pub use stats::{percentile, rank};
pub use traits::{
    AsrStage, AudioOut, Pad, PcmChunk, ReplyEngine, SpeechEnd, StageError, StageNames, TtsStage,
    TurnOut, HEARD, PHRASE,
};
