//! Galion Voice — Perpetual Open MIT License
//!
//! Types-only contract between the duplex student (owns turns + speech) and
//! Hermes (owns knowledge + actions), per V3 §C.3 and the MoshiRAG side-stream
//! pattern. No I/O, no transport. Gateway choice is open decision C5;
//! memory surface is open decision C6.
#![forbid(unsafe_code)]

pub mod client;
/// W21-03 · plan P0.1: `language_hint` / `lang_pin` side-band types.
pub mod lang;

use core::time::Duration;

/// Hard cap on Hermes retrieval before the student fills alone (Q14).
pub const RETRIEVAL_CAP: Duration = Duration::from_millis(3_000);

/// Monotonic turn id within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker { User, Student }

/// One conversational turn as seen by the side-band (inner-monologue text only, never audio).
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: TurnId,
    pub speaker: Speaker,
    pub text: String,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
}

/// Action tier, mirroring persona-plex-relay::speaker::authorize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionTier { Conversation, Personal, Privileged, Money }

/// What the student/engine believes the user wants. Hermes decides whether to act.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Chat,
    Lookup { query: String },
    Action { name: String, tier: ActionTier },
}

impl Intent {
    /// Voice alone may never trigger money actions; privileged needs a 2nd factor.
    pub fn voice_may_request(&self) -> bool {
        !matches!(self, Intent::Action { tier: ActionTier::Money, .. })
    }
}

/// Coarse affect snapshot (valence/arousal in [-1,1], confidence in [0,1]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affect { pub valence: f32, pub arousal: f32, pub confidence: f32 }

impl Affect {
    pub fn clamped(self) -> Self {
        Self {
            valence: self.valence.clamp(-1.0, 1.0),
            arousal: self.arousal.clamp(-1.0, 1.0),
            confidence: self.confidence.clamp(0.0, 1.0),
        }
    }
}

/// Engine -> Hermes event feed.
#[derive(Debug, Clone, PartialEq)]
pub enum ToHermes {
    SessionStart { session: String },
    Turn(Turn),
    Intent { turn: TurnId, intent: Intent },
    Affect { turn: TurnId, affect: Affect },
    SessionEnd,
}

/// Hermes -> engine knowledge injection.
#[derive(Debug, Clone, PartialEq)]
pub enum FromHermes {
    Knowledge { turn: TurnId, text: String, source: Option<String> },
    /// Hermes unreachable or nothing found: student says so honestly, never invents.
    NoKnowledge { turn: TurnId, reason: NoKnowledgeReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKnowledgeReason { Timeout, Unreachable, NotFound, Denied }

/// Classify a reply by elapsed time: anything past the cap is dropped as Timeout.
pub fn within_cap(elapsed: Duration) -> bool { elapsed <= RETRIEVAL_CAP }
