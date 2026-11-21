// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Samantha one-dot page for swarm 16, sector S11.
//!
//! The page is plain HTML, CSS, and JS. This crate serves it on loopback,
//! speaks the assumed WebSocket, and holds the [`ReplyEngine`] seam the
//! HORBM crate can implement later. It does not implement HORBM.

#![forbid(unsafe_code)]

pub mod engine;
pub mod look;
pub mod protocol;
pub mod serve;
pub mod ws;

pub use engine::{FakeReplyEngine, MockEngine, PassiveEngine, ReplyEngine, SessionEngine};
pub use look::{background_at, geometry, look_of, spring_step, DotGeom, DotLook};
pub use protocol::{
    Action, Caption, CaptionSlot, Control, EventKind, Pad, PcmFrame, ProtocolError, ServerEvent,
    TurnOut, VoiceState, SCHEMA, TICK_MS,
};
pub use serve::{measure, Measure, ServeError, Server};

#[cfg(test)]
mod license_test {
    #[test]
    fn license_matches_the_repo() {
        let here = include_bytes!("../LICENSE");
        let repo = include_bytes!("../../../LICENSE");
        assert_eq!(here, repo);
    }
}
