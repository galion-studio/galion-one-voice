//! Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Galion Labs.
//!
//! The side-band mark for one Galion Research request.
//!
//! A start is [`ToHermes::Intent`] with [`Intent::Lookup`]. An end is
//! [`FromHermes`]: knowledge when the answer is inside the cap, or
//! [`FromHermes::NoKnowledge`] when it is not. Those are the same enums
//! [`crate::client::SidebandClient`] already carries. This log does not call
//! that client, so it does not read a wall clock, spawn a thread, or open a
//! socket. The caller passes the virtual millisecond.
//!
//! [`REMEMBER_LOCK`] and [`LEARN_LOCK`] stay `"deny"`. [`LEARN_IN_BAND`] stays
//! false. Posting a mark does not grant either lock and does not teach.
//! [`SidebandConsent::Deny`] writes nothing.

use crate::{FromHermes, Intent, NoKnowledgeReason, ToHermes, TurnId};

/// `LOCKS.toml` `[consent] remember`. Not granted here.
pub const REMEMBER_LOCK: &str = "deny";

/// `LOCKS.toml` `[consent] learn`. Not granted here.
pub const LEARN_LOCK: &str = "deny";

/// Learning does not travel on this band.
pub const LEARN_IN_BAND: bool = false;

/// Label on a merged body. Not a URL and not an address.
pub const MARK_SOURCE: &str = "research";

/// Whether this session writes the research mark.
///
/// The default posts the mark around a request the conductor already sends.
/// [`SidebandConsent::Deny`] writes nothing. Neither arm changes
/// [`REMEMBER_LOCK`], [`LEARN_LOCK`], or [`LEARN_IN_BAND`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebandConsent {
    /// Write a start mark and, when that request finishes, an end mark.
    #[default]
    Post,
    /// Write no mark. The request itself is a separate decision.
    Deny,
}

impl SidebandConsent {
    pub const fn may_post(self) -> bool {
        match self {
            SidebandConsent::Deny => false,
            SidebandConsent::Post => !LEARN_IN_BAND,
        }
    }
}

/// Which edge of the request this mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkEdge {
    Start,
    End,
}

/// What the mark says, in the words the conductor uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// Request opened. Wire: [`ToHermes::Intent`] + [`Intent::Lookup`].
    Lookup { query: String },
    /// Body arrived inside the cap. Wire: [`FromHermes::Knowledge`].
    Merged { text: String },
    /// Cap passed with no usable body. Wire: [`NoKnowledgeReason::Timeout`].
    Dropped,
    /// Barge-in or a resumed turn. Wire: [`NoKnowledgeReason::Cancelled`].
    Cancelled,
    /// Engine answered on the ask. Wire: [`NoKnowledgeReason::Unreachable`].
    Unavailable,
}

/// One posted edge. `posted` is true for every mark that enters the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebandMark {
    pub posted: bool,
    pub turn: u64,
    pub edge: MarkEdge,
    pub kind: MarkKind,
    pub at_ms: u64,
}

impl SidebandMark {
    /// Engine -> side band. Only a start lookup has this shape.
    pub fn outbound(&self) -> Option<ToHermes> {
        match (&self.edge, &self.kind) {
            (MarkEdge::Start, MarkKind::Lookup { query }) => Some(ToHermes::Intent {
                turn: TurnId(self.turn),
                intent: Intent::Lookup {
                    query: query.clone(),
                },
            }),
            _ => None,
        }
    }

    /// Side band -> engine. Only an end has this shape.
    pub fn inbound(&self) -> Option<FromHermes> {
        if self.edge != MarkEdge::End {
            return None;
        }
        let turn = TurnId(self.turn);
        Some(match &self.kind {
            MarkKind::Merged { text } => FromHermes::Knowledge {
                turn,
                text: text.clone(),
                source: Some(MARK_SOURCE.to_string()),
            },
            MarkKind::Dropped => FromHermes::NoKnowledge {
                turn,
                reason: NoKnowledgeReason::Timeout,
            },
            MarkKind::Cancelled => FromHermes::NoKnowledge {
                turn,
                reason: NoKnowledgeReason::Cancelled,
            },
            MarkKind::Unavailable => FromHermes::NoKnowledge {
                turn,
                reason: NoKnowledgeReason::Unreachable,
            },
            MarkKind::Lookup { .. } => return None,
        })
    }
}

/// Marks posted for this session. A start opens a turn. The matching end
/// closes it. A denied session leaves the log empty.
#[derive(Clone, Debug, Default)]
pub struct MarkLog {
    marks: Vec<SidebandMark>,
    open_turn: Option<u64>,
}

impl MarkLog {
    pub const fn new() -> Self {
        Self {
            marks: Vec::new(),
            open_turn: None,
        }
    }

    pub fn marks(&self) -> &[SidebandMark] {
        &self.marks
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// Open a lookup. A denied session does not open, so a later end is a no-op.
    pub fn post_start(&mut self, consent: SidebandConsent, turn: u64, query: &str, at_ms: u64) {
        if !consent.may_post() {
            return;
        }
        self.marks.push(SidebandMark {
            posted: true,
            turn,
            edge: MarkEdge::Start,
            kind: MarkKind::Lookup {
                query: query.to_string(),
            },
            at_ms,
        });
        self.open_turn = Some(turn);
    }

    /// Close the open turn. Does nothing when that turn was never posted.
    pub fn post_end(&mut self, turn: u64, kind: MarkKind, at_ms: u64) {
        if self.open_turn != Some(turn) {
            return;
        }
        self.open_turn = None;
        if matches!(kind, MarkKind::Lookup { .. }) {
            return;
        }
        self.marks.push(SidebandMark {
            posted: true,
            turn,
            edge: MarkEdge::End,
            kind,
            at_ms,
        });
    }
}
