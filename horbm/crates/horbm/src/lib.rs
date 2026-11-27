// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One HORBM turn.
//!
//! Order, fixed:
//!
//! 1. Empathy (layer 3) steps the mood from the heard line and rewrites the
//!    lookup. The feeling name, when one is invited, stays on the mood
//!    metadata. It is not spoken.
//! 2. Person memory, when a diary is attached and `remember` is granted.
//!    The diary is a second file, kept apart from the truth cards. A pin, a name or mood
//!    line, and the session buffer are written there. A milestone is stored
//!    once, under [`SOURCE_MILESTONE`]. The turn then reads that file. A
//!    stored line is spoken only when the utterance is about the person, or
//!    when it asks for the relationship timeline. A world question drops
//!    those hits and does not cite them.
//!    Denied `remember` does not open the file.
//! 3. When a [`RetrievalThread`] is attached, [`Brain::turn`] asks it and
//!    reads the [`MemoryHitCell`]. `Hits` speaks [`MemoryHit::clause`]. `Miss`
//!    speaks [`UNKNOWN_REPLY`] and does not call research. `Idle` is deny or
//!    withdraw: the diary is not read. The card pick runs, and if that misses
//!    the committed clause fixture still answers. Without a thread, cards
//!    answer first through [`CardLookup`]. The phase 4 pick chooses one stored
//!    clause: at most two spoken words first when the clause allows a cut,
//!    then the rest. Lookup and Action turns are marked for Galion Research
//!    and are not posted. [`KnowledgeCards`] reads a directory of
//!    galion-knowledge cards, checks the card-bundle manifest when one is
//!    present, and builds the covering lookup index. It does not write a card
//!    and does not bind a socket. There is no knowledge-ro server in this
//!    repository. Port 7900 is not used here. The 7881 bridge calls this loader
//!    and attaches the retrieval thread.
//! 4. A deep miss on the idle or unthreaded path does not ask Galion Research
//!    inside the turn. The local clause, including [`UNKNOWN_REPLY`], is
//!    split and returned first. The ask is armed behind [`ResearchAsk`] and
//!    starts only after that [`TurnOut`] is dropped. [`Brain::research_follow_up`]
//!    is the later result. It does not rewrite `reply` or `first_clause`.
//!    The caller passes [`RESEARCH_CAP`] (3 seconds). This crate dials nothing
//!    itself. A retrieval `Miss`, a card hit, comfort, and a missing port do not
//!    arm an ask.
//! 5. Anything still unanswered on this turn is exactly [`UNKNOWN_REPLY`].
//!    No gap filler.
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
//! [`Brain::boot`], [`Brain::try_open`], and [`Brain::try_with_cards`] load
//! the configured development persona. The active persona id is
//! `Brain::persona().id()`, loaded from `GALION_HORBM_DEV_PERSONA` /
//! `development.toml` or an explicit persona file. The id comes from
//! `GALION_HORBM_DEV_PERSONA` when that value is non-empty, otherwise from
//! `development.toml`. Persona files are read from `GALION_HORBM_PERSONA_DIR`
//! when that value is non-empty, otherwise from the `persona_dir` key in
//! `development.toml`, relative to that file. A missing directory, a blank
//! id, or a missing file is an error and no brain is built. An explicit path
//! passed to [`Brain::boot`] replaces that file. [`Brain::with_cards`] and
//! [`Brain::open`] keep their existing signatures. They install the same
//! persona when configuration is present, and the stand-in fixture when it
//! is not. They do not panic. [`Brain::admit_move`] is the legal-move port.
//! Layer 4 calls it with the chosen move.
//!
//! Layer 4 runs after that evidence is in hand and before the reply is split
//! into clauses. It picks one move: answer, cite, ask, comfort, offer_topic,
//! or stop. The heuristic is deterministic. An answer with no sourced card
//! becomes ask, and the spoken miss stays [`UNKNOWN_REPLY`]. The truth root
//! is the only card source for answer and cite. [`ResearchRoot`] is optional
//! and unset by default. It reads `GALION_HORBM_RESEARCH_DIR` only: our own
//! private research and development cards (laptop only), to be published
//! later through knowledge.galion.app. A research card never becomes an
//! answer or a citation. Check-ins
//! are off. The offline bandit is off and does not count a move unless learn
//! consent is granted. Callbacks are stored only when remember is granted.
//!
//! [`RetrievalThread`] links `horbm-memory` in-process, off the heartbeat
//! thread, and scores a granted ask with [`horbm_brain::DiaryScan`]. The
//! scan is built when the consent grant opens the store and dropped when
//! the grant falls. `remember` denied does not open it. Do not call
//! [`Brain::turn`] from `Heartbeat::step`: the turn waits for the cell.
//! [`Int8Scorer`] is implemented for the mapped key arena and is not called
//! by the retrieval thread.
//!
//! The configured persona's slower layer holds care, curiosity, help, and
//! expect on [`PersonaInner`]. A host fills an [`InnerHome`] from that
//! persona's baseline numbers and passes the id in. [`Brain::turn`] lets
//! that layer bias the pace and, when the act is comfort, does not arm the
//! research ask. It does not replace the spoken clause. The same GEMI note
//! is stepped on the moment clock. [`Brain::tick_frame`] decays the goals
//! and counts silence. It still does not step the moment.
//!
//! [`LearnPass`] is not part of that order. It is the offline learn pass.
//! Learn consent is [`LearnConfig::granted`]. [`Brain::turn`] does not call it.
//! [`SessionSeal`] keeps one clause until the host calls [`SessionSeal::seal`].
//! The turn does not note or seal it.

#![forbid(unsafe_code)]

mod brain;
mod cards;
mod dialogue;
mod keys;
mod learn;
mod memory;
mod research_style;
mod retrieve;
mod route;

pub use brain::{Brain, MoodMeta, ResearchFollowUp};
pub use cards::{
    blocks_answer, knowledge_dir_from_env, CardError, KnowledgeCards, LoadReport, LoadedPack,
    WINDOWS_KNOWLEDGE_DIR,
};
pub use dialogue::{flag_granted, DialogueConsent, Move, OfflineBandit, TurnSnap, RING_CAP};
pub use galion_emotion_core::standins::memory::{
    DenyMemory, MemoryItem, MemoryKind, MemorySink, PersonMemory, PersonState, Tombstone,
    TrackedName, WriteOutcome,
};
pub use galion_emotion_core::{
    ConductorCue, CoreError as EmotionError, EmotionCore, InnerLife, InnerTurn, IntentMsg,
    MomentState, VoiceBaseline, FRAME_MS as EMOTION_FRAME_MS,
};
pub use galion_research_gateway::RESEARCH_CAP;
pub use galion_sam_empathy::{Intent, PronounClass, FRAME_MS};
pub use horbm_brain::{Answer, Heuristic, MarkKind, SidebandMark};
pub use horbm_inner::{
    DialogueLean, Heard, InnerHome, InnerMsg, MsgDir, Perceived, PersonaInner, Verb, CARE_HOME,
    CHECKIN_GAP_MS, CHECKIN_MS, CURIOSITY_HOME, EXPECT_HOME, HELP_HOME, REMEMBER_LOCK, TICK_MS,
    WAIT_MS,
};
pub use horbm_keys::StringKeyArena;
pub use horbm_persona::{
    LegalMovePort, MoveClaim, MoveVerdict, Persona, PersonaError, PersonaLine, StyleHook,
    LINE_BYTES, UNKNOWN_LINE,
};
pub use learn::{
    ClauseSource, LearnAudit, LearnConfig, LearnError, LearnPass, LearnedRow, SessionSeal,
    LEARN_PACK_CAP, LEARN_PACK_DB, LEARN_PACK_LIVE, LEARN_PACK_NEXT, LEARN_PACK_RETIRING,
};
pub use memory::{
    about_the_person, pin_command, EpisodicStore, TranscriptFile, TranscriptTurn, SOURCE_MILESTONE,
};
pub use research_style::{
    MemoryExample, ResearchHit, ResearchRoot, StyleHint, StylePersona, StyleSearch,
    MAX_RESEARCH_BUNDLE_BYTES, STYLE_HIT_CAP,
};
pub use retrieve::{
    search_person_lexical, CellState, HitPool, Int8Scorer, MemoryHit, MemoryHitCell,
    RetrievalConfig, RetrievalThread, RetrieveError, RetrievedCells, LEARNED_ROW_CAP,
    MEMORY_HIT_CAP,
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

/// What one turn returns to the voice.
///
/// `reply` is the local line. [`Self::first_clause`] is its first spoken
/// piece. Both are set before Galion Research is asked. A later
/// [`Brain::research_follow_up`] does not change them.
///
/// Dropping this value, including the last clone, releases a pending
/// research ask when the turn armed one. The ask does not run while any
/// clone is still held.
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
    /// Drops the release for a follow-up ask. Not part of the spoken line.
    #[doc(hidden)]
    pub(crate) follow: brain::FollowHook,
}
