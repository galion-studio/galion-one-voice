// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback server for the Samantha browser UI.
//!
//! Sector S10. Directory `voice-stack/local-server`. The socket is
//! `galion-samantha-ws/1` on `127.0.0.1:7881`, or another loopback port from
//! `GALION_SAMANTHA_WS` including `::1`. Typed and spoken turns use the HORBM
//! card bridge. A miss is "I don't know." Silero runs out of process when
//! `GALION_VAD_BIN` is set. Piper LJ Speech is the mouth. The s711 clone is
//! selected only when the local reference, the local checkpoint, and CUDA
//! are all present. CPU keeps Piper and logs why.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bind;
mod brain;
mod live;
mod mouth;
pub mod pipeline;
pub mod protocol;
pub mod reply;
pub mod server;
mod voice_http;

pub use audio::{FRAME_MS, MIC_FRAME_SAMPLES, MIC_RATE, SPEAKER_FRAME_SAMPLES, SPEAKER_RATE};
pub use bind::{origin_allowed, require_loopback};
pub use live::live_factory;
pub use pipeline::{unlinked_factory, FakePipeline, PipeInfo, Pipeline, TurnProduct};
pub use reply::{
    FakeReply, HorbmAdapter, HorbmBrain, OpenHorbm, Pad, ReplyEngine, TurnOut, UnlinkedBrain,
};
pub use server::{serve_until_ctrl_c, session_ttl_from_env, start, RefState, Running, ServeError};
