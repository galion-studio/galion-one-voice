// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Moves and the 32-turn ring.
//!
//! The ring holds mood, the focus name, and the topic word for the live
//! talk. It does not write a file and it does not keep the heard line.

use std::collections::VecDeque;

use crate::UNKNOWN_REPLY;

use super::filter::{is_real_source, is_scripted_comfort};

/// How many finished turns the state tracker keeps. Same cap as
/// [`galion_sam_empathy::NameBook::MAX`].
pub const RING_CAP: usize = 32;

/// One legal thing the policy may do on this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Answer,
    Cite,
    Ask,
    Comfort,
    OfferTopic,
    Stop,
}

impl Move {
    pub const ALL: [Move; 6] = [
        Move::Stop,
        Move::Cite,
        Move::OfferTopic,
        Move::Answer,
        Move::Comfort,
        Move::Ask,
    ];

    pub const fn index(self) -> usize {
        match self {
            Move::Stop => 0,
            Move::Cite => 1,
            Move::OfferTopic => 2,
            Move::Answer => 3,
            Move::Comfort => 4,
            Move::Ask => 5,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Move::Answer => "answer",
            Move::Cite => "cite",
            Move::Ask => "ask",
            Move::Comfort => "comfort",
            Move::OfferTopic => "offer_topic",
            Move::Stop => "stop",
        }
    }
}

/// Session consent for this layer. Every field starts denied.
/// Check-ins are not a lock bit. They stay off until the person turns them on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DialogueConsent {
    /// Callback text may be kept in RAM. Default deny. Not the retrieval diary.
    pub remember: bool,
    /// The offline bandit may count a chosen move. Default deny.
    /// Same rule as PR #255: only the flag `grant` is a grant.
    pub learn: bool,
    /// Proactive callback on a short greeting. Default off.
    pub check_ins: bool,
}

impl DialogueConsent {
    /// `grant` after trim grants. `yes`, `true`, empty, and unset deny.
    /// Check-ins stay off. A flag cannot turn them on.
    pub fn from_flags(remember: Option<&str>, learn: Option<&str>) -> Self {
        Self {
            remember: flag_granted(remember),
            learn: flag_granted(learn),
            check_ins: false,
        }
    }
}

/// `true` only for the exact grant string, matching `[horbm] remember_grant`
/// and the learn grant on PR #255 (`GALION_HORBM_LEARN=grant`).
pub fn flag_granted(flag: Option<&str>) -> bool {
    flag.map(str::trim) == Some("grant")
}

/// One finished turn. `clause` is what we spoke, not what we heard.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnSnap {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    pub e: f32,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub chosen: Move,
    pub source: Option<String>,
    pub clause: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced {
    pub source: String,
    pub clause: String,
}

#[derive(Clone, Debug, Default)]
pub struct TurnRing {
    slots: VecDeque<TurnSnap>,
}

impl TurnRing {
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }

    pub fn push(&mut self, snap: TurnSnap) {
        if self.slots.len() >= RING_CAP {
            self.slots.pop_front();
        }
        self.slots.push_back(snap);
    }

    pub fn get(&self, index: usize) -> Option<&TurnSnap> {
        self.slots.get(index)
    }

    /// Newest sourced clause that is allowed to be spoken again.
    pub fn last_sourced(&self) -> Option<Sourced> {
        self.slots.iter().rev().find_map(|snap| {
            let source = snap.source.as_ref()?;
            if !is_real_source(source) {
                return None;
            }
            let clause = snap.clause.trim();
            if clause.is_empty() || clause == UNKNOWN_REPLY || is_scripted_comfort(clause) {
                return None;
            }
            Some(Sourced {
                source: source.clone(),
                clause: clause.to_string(),
            })
        })
    }
}
