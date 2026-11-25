// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Speaking-style hook. Numbers only. No audio.

/// Share of brevity subtracted from a rate of 1. A knob, not a measured curve.
pub const BREVITY_RATE: f32 = 0.15;

/// What the voice reads from the persona. The moment is not an input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleHook {
    pub brevity: f32,
    pub warmth: f32,
    /// Sentences one turn may spend.
    pub max_sentences: u8,
    /// `1 - BREVITY_RATE * brevity`.
    pub rate: f32,
}

pub const fn hook(brevity: f32, warmth: f32, max_sentences: u8) -> StyleHook {
    StyleHook {
        brevity,
        warmth,
        max_sentences,
        rate: 1.0 - BREVITY_RATE * brevity,
    }
}
