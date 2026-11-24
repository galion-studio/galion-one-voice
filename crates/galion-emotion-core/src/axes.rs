// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Four continuous axes. Not a category list.
//!
//! Valence and approach use −1..1. Arousal and dominance use 0..1 so they
//! match `EmotionWire` bytes 3..5 and the MnE contract. A fifth axis would
//! be a new slot in this array, not a new emotion name.

/// Pleasure, −1..1. Same sign as `galion-emotion` `EmotionFrame::valence`.
pub const VALENCE: usize = 0;
/// Activation, 0..1. Same scale as `EmotionWire` arousal, not the bipolar
/// design coordinate in `emotion-v2/emotions.toml`.
pub const AROUSAL: usize = 1;
/// Control, 0..1. Same scale as `EmotionWire` dominance.
pub const DOMINANCE: usize = 2;
/// Approach versus withdrawal, −1..1. Not on the 8-byte wire.
pub const APPROACH: usize = 3;

pub const AXES: usize = 4;

pub const AXIS_NAME: [&str; AXES] = ["valence", "arousal", "dominance", "approach"];

/// Intent pull is an offset from the baseline, allowed past one axis so a
/// pull can still saturate after the baseline is added.
pub const PULL_LIMIT: f32 = 2.0;

pub fn limits(axis: usize) -> (f32, f32) {
    match axis {
        AROUSAL | DOMINANCE => (0.0, 1.0),
        _ => (-1.0, 1.0),
    }
}

pub fn clamp_pos(axis: usize, x: f32) -> f32 {
    let (lo, hi) = limits(axis);
    x.clamp(lo, hi)
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn finite_in(x: f32, lo: f32, hi: f32) -> bool {
    x.is_finite() && x >= lo && x <= hi
}
