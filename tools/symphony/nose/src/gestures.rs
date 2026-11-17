// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Wordless gestures. These are ways of sounding, not the 24 categories.
//!
//! A laugh is the burst channel of amusement (configs/emotions-v2.toml
//! `map.laughter`, and `state::resolve("laughter")` in draft PR 191).
//! A sigh stays a detector across four categories. The other four gestures
//! have no assignment in the sources read for this layer.

/// Gestures this layer will plan. Order is the public list.
pub const GESTURES: [&str; 6] = ["laugh", "sigh", "moan", "gasp", "hum", "mm"];

/// `map.sigh` in configs/emotions-v2.toml. Not a ranking.
pub const SIGH_CANDIDATES: [&str; 4] = ["relief", "sadness", "disappointment", "contentment"];

/// Canonical gesture name, including the aliases the spec already uses.
pub fn canonical(name: &str) -> Option<&'static str> {
    match name {
        "laugh" | "laughter" => Some("laugh"),
        "sigh" => Some("sigh"),
        "moan" => Some("moan"),
        "gasp" => Some("gasp"),
        "hum" | "humming" => Some("hum"),
        "mm" | "mmm" => Some("mm"),
        _ => None,
    }
}

pub fn recipe(gesture: &str) -> &'static str {
    match gesture {
        "laugh" => "laugh-calls",
        "sigh" => "sigh-noise",
        "moan" => "moan-tone",
        "gasp" => "gasp-noise",
        "hum" => "hum-nasal",
        "mm" => "mm-nasal",
        _ => "unknown",
    }
}

/// True when the gesture is phonetically nasal (velum open, mouth closed).
/// A laugh is not. Bachorowski did not measure nasalance.
pub fn opens_velum(gesture: &str) -> bool {
    matches!(gesture, "hum" | "mm")
}
