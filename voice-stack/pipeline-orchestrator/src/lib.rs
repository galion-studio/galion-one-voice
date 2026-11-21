// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Samantha conversation loop.
//!
//! Listen, end of turn, think, speak. Barge-in cancels the speaker without
//! waiting on the chunk queue. The shared stage traits live in
//! [`samantha_traits`]. `horbm::Brain` is not linked; [`samantha_traits::ReplyEngine`]
//! is the seam, and [`samantha_traits::HorbmReply`] is the adapter.
//!
//! Package `galion-sam-orch`, directory `voice-stack/pipeline-orchestrator`.
//! This crate is its own Cargo workspace. It is not a member of the Galion
//! Voice workspace, and it does not call `Heartbeat::step`. A missing `horbm`
//! crate stays an error on [`BrainOpen`]; this loop does not call Hermes.

#![forbid(unsafe_code)]

pub mod brain_route;
pub mod clause;
pub mod clock;
pub mod demo;
pub mod pipeline;
pub mod samantha_traits;
pub mod ui;

pub use brain_route::{asr_to_piper_lines, RouterMouth};
pub use clock::{
    BARGE_IN_MAX_MS, CAPTURE_FRAME_MS, CAPTURE_FRAME_SAMPLES, CAPTURE_RATE_HZ, CHUNK_CHANNEL_BOUND,
    FRAME_CHANNEL_BOUND, HEARTBEAT_MS, HEARTBEAT_PLAY_SAMPLES, PLAYBACK_RATE_HZ, STAGE_BAR_US,
    TURN_CHANNEL_BOUND,
};
pub use pipeline::{Handle, MoodSnap, Phase, Pipeline, Subscribe, TurnReport};
pub use samantha_traits::{
    samantha_ref_from_env, Asr, AudioFrame, AudioIn, AudioOut, BrainOpen, EnergyVad, FakeAsr,
    FakeReply, FakeTts, HoldMood, HorbmBrain, HorbmReply, MemorySpeaker, MoodTick, Pad, Reply,
    ReplyEngine, Transcript, Tts, TurnOut, Vad, VadEvent, SAMANTHA_REF_ENV,
};
