// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback server for the Samantha browser UI.
//!
//! Sector S10 of the local voice stack. The process binds `127.0.0.1` only,
//! serves the one-dot page, and carries mic PCM in and paced TTS PCM out on
//! one WebSocket. The brain, the ear, and the mouth are traits. This crate
//! does not link `horbm`, Piper, or Vosk, and it does not open a reference clip.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bind;
pub mod mood;
pub mod pipeline;
pub mod protocol;
pub mod reply;
pub mod server;

pub use audio::{FRAME_MS, MIC_FRAME_SAMPLES, MIC_RATE, TTS_FRAME_SAMPLES, TTS_RATE};
pub use bind::{origin_is_local, require_ipv4_loopback};
pub use pipeline::{unwired_factory, FakePipeline, PipeInfo, Pipeline, TurnProduct};
pub use reply::{FakeReply, HorbmAdapter, HorbmBrain, OpenHorbm, Pad, ReplyEngine, TurnOut};
pub use server::{serve_until_ctrl_c, start, RefState, Running, ServeError};
