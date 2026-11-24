// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One HORBM turn.
//!
//! Order, fixed:
//!
//! 1. Empathy (layer 3) steps the mood from the heard line and rewrites the
//!    lookup. The feeling name, when one is invited, stays on the mood
//!    metadata. It is not spoken.
//! 2. Cards answer first, through [`CardLookup`]. The phase 4 pick in
//!    `horbm-brain` chooses one stored clause from that lookup: at most two
//!    spoken words first when the clause allows a cut, then the rest. A miss
//!    is the stored persona clause [`UNKNOWN_REPLY`]. Lookup and Action turns
//!    are marked for Galion Research and are not posted. [`KnowledgeCards`]
//!    reads a directory of galion-knowledge cards, checks the card-bundle
//!    manifest when one is present, and builds the covering lookup index. It
//!    does not write a card, does not open `horbm-memory`, and does not bind
//!    a socket. There is no knowledge-ro server in this repository. Port 7900
//!    is not used here. The 7881 bridge calls this loader.
//! 3. A deep miss may ask the research gateway. The caller passes
//!    [`RESEARCH_CAP`] (3 seconds). This crate dials nothing itself.
//! 4. Anything still unanswered is exactly [`UNKNOWN_REPLY`]. No gap filler.
//!
//! Beside that turn, [`Brain::step_moment`] advances the moment-to-moment
//! emotion core one 80 ms frame. Empathy mood and the moment are separate:
//! `tick_frame` does not step the core, and `step_moment` does not rewrite
//! the mood. [`Brain::conductor_cue`] is what the conductor reads.
//!
//! [`Brain::inner_turn`] updates cares, wants, and expects and emits one
//! GEMI intent. [`Brain::inner_nudge`] steps that message through the moment
//! core. Neither call rewrites the empathy mood or the voice baseline.
//! Learning a new care stays off while `LOCKS.toml` `[consent] learn` is deny.
//!
//! [`Brain::boot`] reads a persona file once. The baseline numbers and the
//! inner-life seed come from that file. With no path, the stand-in baseline
//! and the stand-in seed stay. [`Brain::admit_move`] is the legal-move port.

#![forbid(unsafe_code)]

mod brain;
mod cards;
mod route;

pub use brain::{Brain, MoodMeta};
pub use cards::{
    knowledge_dir_from_env, CardError, KnowledgeCards, LoadReport, LoadedPack,
    WINDOWS_KNOWLEDGE_DIR,
};
pub use galion_emotion_core::{
    ConductorCue, CoreError as EmotionError, EmotionCore, InnerLife, InnerTurn, IntentMsg,
    MomentState, VoiceBaseline, FRAME_MS as EMOTION_FRAME_MS,
};
pub use galion_research_gateway::RESEARCH_CAP;
pub use galion_sam_empathy::{Intent, PronounClass, FRAME_MS};
pub use horbm_brain::{Answer, Heuristic, MarkKind, SidebandMark};
pub use horbm_persona::{
    LegalMovePort, MoveClaim, MoveVerdict, Persona, PersonaError, StyleHook, UNKNOWN_LINE,
};
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
    /// First spoken piece of `reply`. At most two words when the clause allows a cut.
    pub first_clause: String,
    /// The rest of `reply` after [`Self::first_clause`]. Empty when the clause stays whole.
    pub rest: String,
    /// Galion Research side-band mark. `posted` is false until phase 6.
    pub sideband: Option<SidebandMark>,
}
