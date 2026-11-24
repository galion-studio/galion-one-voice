// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback server for the Samantha browser UI.
//!
//! Sector S10. Directory `voice-stack/local-server`. The socket is
//! `galion-samantha-ws/1` on `127.0.0.1:7881`, or another loopback port from
//! `GALION_SAMANTHA_WS` including `::1`. Typed and spoken turns use the HORBM
//! card bridge. A miss is "I don't know." Silero runs out of process when
//! `GALION_VAD_BIN` is set. A turn is [`horbm_voice::VoiceLoop`]: ear, then
//! `horbm::Brain::turn`, then the mouth selected at boot. CPU is the default
//! device. The in-process tone is the mouth unless this crate is built with
//! `--features piper`. The s711 clone is selected only when the local
//! reference, the local checkpoint, and an explicit cuda opt-in are all present.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bind;
mod brain;
mod live;
mod mouth;
mod native;
pub mod pipeline;
pub mod protocol;
pub mod reply;
pub mod server;
mod voice_http;

pub use audio::{FRAME_MS, MIC_FRAME_SAMPLES, MIC_RATE, SPEAKER_FRAME_SAMPLES, SPEAKER_RATE};
pub use bind::{origin_allowed, require_loopback};
pub use horbm_voice::{EarEvent, TickEvent, BARGE_IN_MAX_MS};
pub use live::{gated_mouth_factory, live_factory};
pub use native::{NativeSession, BOOT_BRAIN, BOOT_MOUTH_DEVICE};
pub use pipeline::{unlinked_factory, FakePipeline, PipeInfo, Pipeline, TurnProduct};
pub use reply::{
    FakeReply, HorbmAdapter, HorbmBrain, OpenHorbm, Pad, ReplyEngine, TurnOut, UnlinkedBrain,
};
pub use server::{serve_until_ctrl_c, session_ttl_from_env, start, RefState, Running, ServeError};
