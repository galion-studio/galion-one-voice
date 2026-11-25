// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The persona the duplex loop seats.
//!
//! [`ConfiguredPersona`] is loaded from the development persona
//! ([`horbm::Persona::load_development`]) or from an explicit persona file.
//! The historical [`SamanthaPersona`] forward stays so soak can keep
//! constructing it. There is no `deprecated` attribute this round.

use crate::ports::{Persona, Style, DEV_PERSONA};
use crate::LoopError;

/// Brevity the historical forward still publishes.
///
/// The live loop reads brevity from the configured persona file. Soak still
/// reaches this number through [`SamanthaPersona`]. There is no `deprecated`
/// attribute this round.
pub const SAMANTHA_BREVITY: f32 = 0.55;
/// Warmth the historical forward still publishes.
///
/// See [`SAMANTHA_BREVITY`].
pub const SAMANTHA_WARMTH: f32 = 0.8;
/// Sentences one turn may spend on the historical forward.
///
/// See [`SAMANTHA_BREVITY`].
pub const SAMANTHA_SENTENCES: u8 = 2;

/// The persona the loop seats. Id, voice, and style come from the file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfiguredPersona {
    persona: horbm::Persona,
}

impl ConfiguredPersona {
    /// Seat a persona that was already parsed or loaded.
    #[must_use]
    pub fn from_persona(persona: horbm::Persona) -> Self {
        Self { persona }
    }

    /// The development persona for this process.
    ///
    /// [`horbm::Persona::load_development`] follows `GALION_HORBM_DEV_PERSONA`,
    /// otherwise `development.toml`.
    pub fn development() -> Result<Self, horbm::PersonaError> {
        horbm::Persona::load_development().map(Self::from_persona)
    }

    /// Baseline copied from the file. The emotion step does not write it.
    #[must_use]
    pub fn baseline(&self) -> galion_emotion_core::VoiceBaseline {
        self.persona.baseline()
    }
}

impl Persona for ConfiguredPersona {
    fn id(&self) -> &str {
        self.persona.id()
    }

    fn voice(&self) -> &str {
        self.persona.voice()
    }

    fn style(&self) -> Style {
        let hook = self.persona.style();
        Style {
            brevity: hook.brevity,
            warmth: hook.warmth,
            max_sentences: hook.max_sentences,
            rate: hook.rate,
        }
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

/// Historical development speaker. Soak still constructs this type.
///
/// The live loop builds [`ConfiguredPersona`]. This forward keeps the
/// previous id, voice, and style numbers so that crate compiles. There is
/// no `deprecated` attribute this round.
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

/// The configured persona, when `id` is the one development boot names.
///
/// Any other id is refused, including a persona file that exists but is not
/// the one `GALION_HORBM_DEV_PERSONA` or `development.toml` names. The
/// refusal does not list ids.
pub fn open_persona(id: &str) -> Result<ConfiguredPersona, LoopError> {
    let persona = ConfiguredPersona::development()
        .map_err(|_| LoopError::Persona("the configured persona is not loaded"))?;
    if id == persona.id() {
        Ok(persona)
    } else {
        Err(LoopError::Persona("the id is not the configured persona"))
    }
}
