// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Projection of the continuous feeling onto the four scalar bytes of
//! `EmotionWire` (`galion-emotion/src/band.rs`).
//!
//! The layout this matches is:
//! valence i8 = round(valence * 127), arousal u8 = round(arousal * 255),
//! dominance u8 = round(dominance * 255), confidence u8 = round(confidence * 255).
//!
//! The three-bit affect label in that wire is not filled. A Cowen pin does
//! not fit in three bits, and this crate does not invent a packing for it.
//! The projection is lossy. It is not the JSON/TOML mirror.

use crate::model::Feeling;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmotionWireSample {
    pub valence_i8: i8,
    pub arousal_u8: u8,
    pub dominance_u8: u8,
    pub confidence_u8: u8,
}

pub fn project_emotion_wire(feeling: &Feeling, confidence: f64) -> EmotionWireSample {
    let arousal = feeling.arousal.clamp(0.0, 1.0);
    let valence = feeling.valence.clamp(-1.0, 1.0);
    let dominance = feeling.dominance.clamp(0.0, 1.0);
    let confidence = confidence.clamp(0.0, 1.0);
    EmotionWireSample {
        valence_i8: (valence * 127.0).round() as i8,
        arousal_u8: (arousal * 255.0).round() as u8,
        dominance_u8: (dominance * 255.0).round() as u8,
        confidence_u8: (confidence * 255.0).round() as u8,
    }
}
