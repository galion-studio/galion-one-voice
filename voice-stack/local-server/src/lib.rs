// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback server for the local voice UI.
//!
//! Sector S10. Directory `voice-stack/local-server`. The socket is
//! `galion-samantha-ws/1` on `127.0.0.1:7881`, or another loopback port from
//! `GALION_VOICE_WS` including `::1`. `GALION_SAMANTHA_WS` is a deprecated
//! alias and logs one warning. Typed and spoken turns use the HORBM
//! card bridge. A miss is "I don't know." Silero runs out of process when
//! `GALION_VAD_BIN` is set. A turn is [`horbm_voice::VoiceLoop`]: ear, then
//! `horbm::Brain::turn`, then the mouth selected at boot. CPU is the default
//! device. The in-process tone is the mouth unless this crate is built with
//! `--features piper`. `GALION_MOUTH=native` selects the HORBM text mouth:
//! `TurnOut.first_clause`, then `rest`, on this socket, and no PCM. Unset,
//! `piper`, or an unknown value keeps today's mouth. The s711 clone is
//! selected only when the local reference, the local checkpoint, and an
//! explicit cuda opt-in are all present.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bind;
mod brain;
pub mod env_alias;
mod host;
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
pub use brain::knowledge_dir;
pub use horbm_voice::{EarEvent, TickEvent, BARGE_IN_MAX_MS};
pub use host::{HostConfig, HostSession, HostSources, TurnLog};
pub use live::{gated_mouth_factory, live_factory, live_factory_with};
pub use native::{NativeSession, BOOT_BRAIN, BOOT_MOUTH_DEVICE};
pub use pipeline::{unlinked_factory, FakePipeline, PipeInfo, Pipeline, TurnProduct};
pub use reply::{
    FakeReply, HorbmAdapter, HorbmBrain, OpenHorbm, Pad, ReplyEngine, TurnOut, UnlinkedBrain,
};
pub use server::{
    serve_until_ctrl_c, session_ttl_from_env, start, RefState, Running, ServeError, Stop,
};

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// [`start`] with the booted host. Shutdown seals the session.
pub async fn start_with_host(
    addr: SocketAddr,
    ui_dir: PathBuf,
    host: Arc<HostSession>,
    voice_ref: RefState,
    require_session: bool,
    ttl: Duration,
) -> Result<Running, ServeError> {
    let factory = live_factory_with(Arc::clone(&host));
    server::start_bound(
        addr,
        ui_dir,
        factory,
        voice_ref,
        require_session,
        ttl,
        Some(host),
    )
    .await
}
