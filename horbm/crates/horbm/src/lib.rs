// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One HORBM turn.
//!
//! Order, fixed:
//!
//! 1. Empathy (layer 3) steps the mood from the heard line and rewrites the
//!    lookup. The feeling name, when one is invited, stays on the mood
//!    metadata. It is not spoken.
//! 2. Cards answer first, through [`CardLookup`]. This crate does not write
//!    a card, does not open `horbm-memory`, and does not bind a socket.
//!    There is no knowledge-ro server in this repository. Port 7900 is not
//!    used here. The live loader stays the read-only loader in
//!    `galion-sam-horbm`.
//! 3. A deep miss may ask the research gateway. The caller passes
//!    [`RESEARCH_CAP`] (3 seconds). This crate dials nothing itself.
//! 4. Anything still unanswered is exactly [`UNKNOWN_REPLY`]. No gap filler.

#![forbid(unsafe_code)]

mod brain;
mod route;

pub use brain::{Brain, MoodMeta};
pub use galion_research_gateway::RESEARCH_CAP;
pub use galion_sam_empathy::{Intent, PronounClass, FRAME_MS};
pub use route::{
    is_deep_question, route_heard, AskOutcome, CardHit, CardLookup, Gap, GapReason, ResearchAsk,
    RouteKind, RouteOut, Speakable, GAP_ADVICE, UNKNOWN_REPLY,
};

/// Pleasure-arousal-dominance. Rest is arousal 0.3, valence 0.25, dominance 0.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };
}

/// What one turn returns to the voice. `reply` is the only spoken text.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
    pub mood_meta: MoodMeta,
}
