// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The steady character.
//!
//! A persona file is TOML, schema [`SCHEMA`], read once at boot. Traits are
//! open numbers. Honesty is fixed: never fabricate, and the miss is
//! [`UNKNOWN_LINE`]. Boundaries are open tokens the legal-move port reads.
//! Style is a voice hook with no audio. Baseline numbers are copied into a
//! [`VoiceBaseline`]. Care, want, and expect seed [`InnerLife`] when each
//! one is a single lowercase word.
//!
//! [`Persona::verdict`] and [`Persona::style`] do not allocate.
//! [`TURN_ALLOC_BUDGET`] is 0. The same input is the same output.
//! CPU only. No model, no audio, no thread.
#![forbid(unsafe_code)]

mod boot;
mod legal;
mod load;
mod style;

use galion_emotion_core::{InnerLife, VoiceBaseline, APPROACH, AROUSAL, DOMINANCE, VALENCE};

pub use boot::BootConfig;
pub use legal::{LegalMovePort, MoveClaim, MoveVerdict, FAKE_SYMPATHY, UNSOURCED_FACT};
pub use style::{StyleHook, BREVITY_RATE};

pub const SCHEMA: &str = "galion-horbm-persona/1";
pub const VERSION: u32 = 1;
/// Capacity of the trait table. Not a list of trait names.
pub const TRAIT_SLOTS: usize = 8;
/// Capacity of the boundary list. Not a list of move kinds.
pub const BOUNDARY_SLOTS: usize = 8;
pub const TOKEN_BYTES: usize = 16;
pub const BOUNDARY_BYTES: usize = 24;
/// The spoken miss. The same sentence the turn already uses.
pub const UNKNOWN_LINE: &str = "I don't know.";
/// Bytes of the shipped `personas/samantha.toml` file. [`Persona::samantha`] parses them.
/// Which id may boot is [`BootConfig`], not this constant.
pub const SAMANTHA_TOML: &str = include_str!("../personas/samantha.toml");
/// Heap allocations [`Persona::verdict`] and [`Persona::style`] may make.
pub const TURN_ALLOC_BUDGET: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonaError {
    Unreadable,
    Invalid(&'static str),
}

impl std::fmt::Display for PersonaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable => f.write_str("cannot read persona file"),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for PersonaError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TraitSlot {
    key: [u8; TOKEN_BYTES],
    value: f32,
}

impl TraitSlot {
    pub const EMPTY: Self = Self {
        key: [0; TOKEN_BYTES],
        value: 0.0,
    };

    pub const fn new(key: &[u8], value: f32) -> Self {
        Self {
            key: pack_bytes(key),
            value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Boundary {
    token: [u8; BOUNDARY_BYTES],
}

impl Boundary {
    pub const EMPTY: Self = Self {
        token: [0; BOUNDARY_BYTES],
    };

    pub const fn new(token: &[u8]) -> Self {
        Self {
            token: pack_bytes(token),
        }
    }
}

/// The character, packed. Copy. The hot path borrows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Persona {
    id: [u8; TOKEN_BYTES],
    voice: [u8; TOKEN_BYTES],
    traits: [TraitSlot; TRAIT_SLOTS],
    trait_len: u8,
    boundaries: [Boundary; BOUNDARY_SLOTS],
    boundary_len: u8,
    style: StyleHook,
    valence: f32,
    arousal: f32,
    dominance: f32,
    approach: f32,
    pitch_centre_hz: f32,
    care: [u8; TOKEN_BYTES],
    want: [u8; TOKEN_BYTES],
    expect: [u8; TOKEN_BYTES],
}

impl Persona {
    /// The emotion-core fixture: `VoiceBaseline::standin` and the seeded
    /// inner life, plus the honesty rules. HORBM boot loads [`Self::samantha`].
    pub const fn standin() -> Self {
        let base = VoiceBaseline::standin();
        let style = style::hook(0.5, 0.4, 2);
        Self {
            id: base.persona,
            voice: base.voice,
            traits: [
                TraitSlot::new(b"steadiness", 0.5),
                TraitSlot::new(b"warmth", 0.4),
                TraitSlot::EMPTY,
                TraitSlot::EMPTY,
                TraitSlot::EMPTY,
                TraitSlot::EMPTY,
                TraitSlot::EMPTY,
                TraitSlot::EMPTY,
            ],
            trait_len: 2,
            boundaries: [
                Boundary::new(b"fake-sympathy"),
                Boundary::new(b"invented-past"),
                Boundary::new(b"unsourced-fact"),
                Boundary::EMPTY,
                Boundary::EMPTY,
                Boundary::EMPTY,
                Boundary::EMPTY,
                Boundary::EMPTY,
            ],
            boundary_len: 3,
            style,
            valence: base.pos[VALENCE],
            arousal: base.pos[AROUSAL],
            dominance: base.pos[DOMINANCE],
            approach: base.pos[APPROACH],
            pitch_centre_hz: base.pitch_centre_hz,
            care: pack_bytes(b"heard"),
            want: pack_bytes(b"answer"),
            expect: pack_bytes(b"pause"),
        }
    }

    /// Parse [`SAMANTHA_TOML`]. Development boot uses [`BootConfig`], not this name.
    pub fn samantha() -> Result<Self, PersonaError> {
        Self::parse(SAMANTHA_TOML)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, PersonaError> {
        load::load(path)
    }

    pub fn parse(text: &str) -> Result<Self, PersonaError> {
        load::parse(text)
    }

    pub fn to_toml(&self) -> String {
        load::to_toml(self)
    }

    pub fn id(&self) -> &str {
        as_str(&self.id)
    }

    /// True when this persona's id is the one `cfg` names.
    ///
    /// Any schema-valid persona matches. No id is special. An unset config
    /// never reaches here: [`BootConfig`] refuses to build one.
    #[must_use]
    pub fn is_configured_boot(&self, cfg: &BootConfig) -> bool {
        self.id() == cfg.id()
    }

    pub fn voice(&self) -> &str {
        as_str(&self.voice)
    }

    pub fn trait_len(&self) -> usize {
        usize::from(self.trait_len)
    }

    pub fn trait_at(&self, index: usize) -> Option<(&str, f32)> {
        if index >= self.trait_len() {
            return None;
        }
        let slot = &self.traits[index];
        Some((as_str(&slot.key), slot.value))
    }

    pub fn trait_value(&self, name: &str) -> Option<f32> {
        let want = name.as_bytes();
        for i in 0..self.trait_len() {
            if prefix(&self.traits[i].key) == want {
                return Some(self.traits[i].value);
            }
        }
        None
    }

    pub fn boundary_len(&self) -> usize {
        usize::from(self.boundary_len)
    }

    pub fn boundary_at(&self, index: usize) -> Option<&str> {
        if index >= self.boundary_len() {
            return None;
        }
        Some(as_str(&self.boundaries[index].token))
    }

    pub fn forbids(&self, token: &[u8]) -> bool {
        let n = self.boundary_len();
        self.boundaries[..n]
            .iter()
            .any(|slot| prefix(&slot.token) == token)
    }

    pub fn unknown_line(&self) -> &'static str {
        UNKNOWN_LINE
    }

    #[must_use]
    pub fn style(&self) -> StyleHook {
        self.style
    }

    #[must_use]
    pub fn verdict(&self, claim: MoveClaim<'_>) -> MoveVerdict {
        legal::decide(self, claim)
    }

    pub fn valence(&self) -> f32 {
        self.valence
    }
    pub fn arousal(&self) -> f32 {
        self.arousal
    }
    pub fn dominance(&self) -> f32 {
        self.dominance
    }
    pub fn approach(&self) -> f32 {
        self.approach
    }
    pub fn pitch_centre_hz(&self) -> f32 {
        self.pitch_centre_hz
    }

    /// The baseline values, copied into the emotion core's type.
    pub fn baseline(&self) -> VoiceBaseline {
        VoiceBaseline {
            persona: self.id,
            voice: self.voice,
            pos: [self.valence, self.arousal, self.dominance, self.approach],
            pitch_centre_hz: self.pitch_centre_hz,
        }
    }

    pub fn care(&self) -> &str {
        as_str(&self.care)
    }

    pub fn want(&self) -> &str {
        as_str(&self.want)
    }

    pub fn expect_token(&self) -> &str {
        as_str(&self.expect)
    }

    /// Seed cares, wants, and expects from the three tokens.
    pub fn inner_life(&self) -> InnerLife {
        InnerLife::from_prose(self.care(), self.want(), self.expect_token())
    }
}

pub(crate) const fn pack_bytes<const N: usize>(s: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < s.len() && i < N {
        out[i] = s[i];
        i += 1;
    }
    out
}

pub(crate) fn prefix(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

pub(crate) fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(prefix(bytes)).unwrap_or("")
}
