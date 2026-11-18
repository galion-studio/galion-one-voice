// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Planning alphabet. Not a grapheme-to-phoneme model and not a claim about
//! English phonotactics. Unknown symbols stay unknown (viseme X).
//!
//! Viseme letters are Rhubarb Lip Sync 1.13.0, README: A P/B/M, B most other
//! consonants and EE, C EH/AE, D AA, E AO/ER, F UW/OW/W, G F/V, H L, X rest.
//! Jaw, tongue and lip classes follow the direction in Lindblom & Sundberg
//! 1971. The ±1 units are category codes for the blender, not millimetres
//! and not SPARC template units.

use crate::direction::{LINDBLOM_SUNDBERG, RHUBARB};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Units {
    pub jaw: f64,
    pub front: f64,
    pub height: f64,
    pub lips: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhoneClass {
    pub symbol: String,
    pub known: bool,
    pub vowel: bool,
    pub viseme: &'static str,
    pub jaw: &'static str,
    pub tongue_front: &'static str,
    pub tongue_height: &'static str,
    pub tongue_tip: &'static str,
    pub lips: &'static str,
    pub units: Option<Units>,
}

pub fn cite() -> &'static [&'static str] {
    &[RHUBARB, LINDBLOM_SUNDBERG]
}

pub fn classify(symbol: &str) -> PhoneClass {
    let symbol = normalize(symbol);
    if let Some(row) = ROW.iter().find(|r| r.symbols.iter().any(|s| *s == symbol)) {
        return PhoneClass {
            symbol,
            known: true,
            vowel: row.vowel,
            viseme: row.viseme,
            jaw: row.jaw,
            tongue_front: row.front,
            tongue_height: row.height,
            tongue_tip: row.tip,
            lips: row.lips,
            units: row.units,
        };
    }
    PhoneClass {
        symbol,
        known: false,
        vowel: false,
        viseme: "X",
        jaw: "unspecified",
        tongue_front: "unspecified",
        tongue_height: "unspecified",
        tongue_tip: "unspecified",
        lips: "unspecified",
        units: None,
    }
}

fn normalize(symbol: &str) -> String {
    let s = symbol.trim().to_ascii_lowercase();
    s.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

struct Row {
    symbols: &'static [&'static str],
    vowel: bool,
    viseme: &'static str,
    jaw: &'static str,
    front: &'static str,
    height: &'static str,
    tip: &'static str,
    lips: &'static str,
    units: Option<Units>,
}

struct Axis {
    label: &'static str,
    unit: f64,
}

const fn axis(label: &'static str, unit: f64) -> Axis {
    Axis { label, unit }
}

const fn vowel(
    symbols: &'static [&'static str],
    viseme: &'static str,
    jaw: Axis,
    front: Axis,
    height: Axis,
    lips: Axis,
) -> Row {
    Row {
        symbols,
        vowel: true,
        viseme,
        jaw: jaw.label,
        front: front.label,
        height: height.label,
        tip: "neutral",
        lips: lips.label,
        units: Some(Units {
            jaw: jaw.unit,
            front: front.unit,
            height: height.unit,
            lips: lips.unit,
        }),
    }
}

const fn cons(
    symbols: &'static [&'static str],
    viseme: &'static str,
    jaw: &'static str,
    tip: &'static str,
    lips: &'static str,
) -> Row {
    Row {
        symbols,
        vowel: false,
        viseme,
        jaw,
        front: "unspecified",
        height: "unspecified",
        tip,
        lips,
        units: None,
    }
}

const ROW: &[Row] = &[
    vowel(
        &["aa"],
        "D",
        axis("open", 1.0),
        axis("back", -1.0),
        axis("low", -1.0),
        axis("neutral", 0.0),
    ),
    vowel(
        &["ae"],
        "C",
        axis("open", 1.0),
        axis("front", 1.0),
        axis("low", -1.0),
        axis("spread", 1.0),
    ),
    vowel(
        &["eh"],
        "C",
        axis("mid", 0.0),
        axis("front", 1.0),
        axis("mid", 0.0),
        axis("spread", 1.0),
    ),
    vowel(
        &["iy", "ih", "ee"],
        "B",
        axis("close", -1.0),
        axis("front", 1.0),
        axis("high", 1.0),
        axis("spread", 1.0),
    ),
    vowel(
        &["uw"],
        "F",
        axis("close", -1.0),
        axis("back", -1.0),
        axis("high", 1.0),
        axis("round", -1.0),
    ),
    vowel(
        &["ow"],
        "F",
        axis("mid", 0.0),
        axis("back", -1.0),
        axis("mid", 0.0),
        axis("round", -1.0),
    ),
    vowel(
        &["w"],
        "F",
        axis("close", -1.0),
        axis("back", -1.0),
        axis("high", 1.0),
        axis("round", -1.0),
    ),
    vowel(
        &["ao"],
        "E",
        axis("open", 1.0),
        axis("back", -1.0),
        axis("mid", 0.0),
        axis("round", -1.0),
    ),
    vowel(
        &["er"],
        "E",
        axis("mid", 0.0),
        axis("mid", 0.0),
        axis("mid", 0.0),
        axis("round", -1.0),
    ),
    cons(&["m", "b", "p"], "A", "close", "unspecified", "closed"),
    cons(&["f", "v"], "G", "close", "unspecified", "labiodental"),
    cons(&["l"], "H", "mid", "up", "neutral"),
    cons(
        &["s", "t", "d", "k", "g", "n", "z"],
        "B",
        "close",
        "unspecified",
        "neutral",
    ),
    cons(&["sil", "sp", "pau"], "X", "rest", "unspecified", "rest"),
];
