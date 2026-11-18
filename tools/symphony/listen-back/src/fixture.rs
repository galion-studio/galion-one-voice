// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Built-in fixture scenarios. No audio is stored. The numbers are the
//! scenario's intended shape, not measurements and not emotion-render steps.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BreathSpan {
    pub start_s: f64,
    pub dur_s: f64,
}

#[derive(Clone, Debug)]
pub struct Fixture {
    pub id: &'static str,
    pub text: &'static str,
    /// Name before the contract map. `laughter` is burst amusement.
    pub name: &'static str,
    pub baseline_f0_hz: f64,
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f3_hz: f64,
    pub pitch_st: f64,
    pub speech_s: f64,
    pub breaths: Vec<BreathSpan>,
}

fn breath(dur_s: f64) -> Vec<BreathSpan> {
    vec![BreathSpan {
        start_s: 0.0,
        dur_s,
    }]
}

/// Synthetic tones. The baseline is a test oscillator, not a person.
pub fn builtin() -> Vec<Fixture> {
    let voice = |id, text, name, pitch_st, breaths| Fixture {
        id,
        text,
        name,
        baseline_f0_hz: 180.0,
        f1_hz: 700.0,
        f2_hz: 1200.0,
        f3_hz: 2800.0,
        pitch_st,
        speech_s: 0.80,
        breaths,
    };
    vec![
        voice("neutral-hold", "Hello.", "neutral", 0.0, Vec::new()),
        voice("sadness-fall", "I am sorry.", "sadness", -2.0, breath(0.16)),
        voice(
            "embarrassment-word",
            "Oh. Sorry.",
            "embarrassment",
            1.5,
            breath(0.16),
        ),
        voice("laughter-burst", "Ha.", "laughter", 0.0, breath(0.16)),
    ]
}
