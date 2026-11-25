// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Legal-move port.
//!
//! The dialogue manager calls [`LegalMovePort::verdict`] with one candidate.
//! This module does not name that manager's types. A move is a few flags
//! and an open tag. The persona's boundary list decides.

use crate::Persona;

/// Boundary token for a stock sympathy script.
pub const FAKE_SYMPATHY: &[u8] = b"fake-sympathy";
/// Boundary token for a fact that has no card behind it.
pub const UNSOURCED_FACT: &[u8] = b"unsourced-fact";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveVerdict {
    Allow,
    Forbid,
}

/// One candidate the filter is considering. Borrowed. Not stored.
#[derive(Clone, Copy, Debug)]
pub struct MoveClaim<'a> {
    /// Open label, compared with the boundary tokens.
    pub tag: &'a [u8],
    pub claims_fact: bool,
    pub has_card: bool,
    pub sympathy_script: bool,
}

/// What the dialogue manager's legal-move filter calls.
///
/// ```
/// use horbm_persona::{LegalMovePort, MoveClaim, MoveVerdict, Persona};
/// let persona = Persona::standin();
/// let claim = MoveClaim {
///     tag: b"fact",
///     claims_fact: true,
///     has_card: false,
///     sympathy_script: false,
/// };
/// let port: &dyn LegalMovePort = &persona;
/// assert_eq!(port.verdict(claim), MoveVerdict::Forbid);
/// ```
pub trait LegalMovePort {
    fn verdict(&self, claim: MoveClaim<'_>) -> MoveVerdict;
}

pub(crate) fn decide(persona: &Persona, claim: MoveClaim<'_>) -> MoveVerdict {
    if claim.sympathy_script && persona.forbids(FAKE_SYMPATHY) {
        return MoveVerdict::Forbid;
    }
    if claim.claims_fact && !claim.has_card && persona.forbids(UNSOURCED_FACT) {
        return MoveVerdict::Forbid;
    }
    if persona.forbids(claim.tag) {
        return MoveVerdict::Forbid;
    }
    MoveVerdict::Allow
}

impl LegalMovePort for Persona {
    fn verdict(&self, claim: MoveClaim<'_>) -> MoveVerdict {
        decide(self, claim)
    }
}
