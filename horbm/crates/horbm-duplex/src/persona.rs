// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Development persona. Samantha is loaded. Aria is a parked name.
//!
//! The style numbers are the ones the virtual-clock gate already speaks
//! with. The persona crate that reads a TOML file is not on this branch.

use crate::ports::{Persona, Style, DEV_PERSONA};
use crate::LoopError;

/// Brevity the gate uses. `Style::rate` is `1 - 0.15 * brevity`.
pub const SAMANTHA_BREVITY: f32 = 0.55;
/// Warmth the gate uses.
pub const SAMANTHA_WARMTH: f32 = 0.8;
/// Sentences one turn may spend.
pub const SAMANTHA_SENTENCES: u8 = 2;

/// Aria's id. She is not constructed and not selected.
pub const ARIA_ID: &str = "aria";
/// Owner decision for this phase: Aria stays inactive.
pub const ARIA_ACTIVE: bool = false;

/// The only persona the live loop builds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SamanthaPersona;

impl Persona for SamanthaPersona {
    fn id(&self) -> &str {
        DEV_PERSONA
    }

    fn voice(&self) -> &str {
        DEV_PERSONA
    }

    fn style(&self) -> Style {
        Style::from_brevity(SAMANTHA_BREVITY, SAMANTHA_WARMTH, SAMANTHA_SENTENCES)
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

/// Samantha, or a refusal. Aria is refused here, before [`crate::Duplex::new`]
/// checks the id again.
pub fn open_persona(id: &str) -> Result<SamanthaPersona, LoopError> {
    if id == DEV_PERSONA {
        return Ok(SamanthaPersona);
    }
    if id == ARIA_ID {
        return Err(LoopError::Persona("aria is parked and is not loaded"));
    }
    Err(LoopError::Persona(
        "dev phase speaks as samantha and with the samantha voice",
    ))
}
