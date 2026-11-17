// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Nose and wordless sounds for one utterance.
//!
//! Two outputs, kept apart:
//! - **Nose.** Velum opening on the 80 ms tick, and a nasal murmur whose level
//!   follows that opening. The speaker baseline is the resting velum. An open
//!   feeling is an input. It moves the velum only through a config gain, which
//!   ships at zero because no per-category nasalance was read.
//! - **Bursts.** Laughs, sighs, moans, gasps, hums and "mm" on their own track.
//!   The planner places them from intent and, for amusement only, from the
//!   burst track of the open feeling. The same feeling decides the shape that
//!   a cited recipe actually has. Prosody amusement does not become a laugh.
//!
//! Detection is not reimplemented. Reading parses the JSON that
//! `galion-articulation` already writes (draft PR 191): the laughter worker
//! and the decode burst track. Samples of a burst, when the caller passes
//! them, are that speaker's own reference at runtime. Nothing here commits audio.
//!
//! Directions are cited in `docs/galion-voice-v2/symphony/NOSE.md`.
//! Magnitudes live in `configs/symphony/nose.toml` and are not measurements.
#![forbid(unsafe_code)]

pub mod baseline;
pub mod bursts;
pub mod config;
pub mod contract;
pub mod feeling;
pub mod gestures;
pub mod handoff;
pub mod inventory;
pub mod plan;
pub mod read;
pub mod synth;
pub mod wav;

/// Symphony heartbeat. LOCKS `clock.frame_ms` = 80.
pub const TICK_S: f64 = 0.08;
pub const TICK_MS: u32 = 80;

/// Papers used for a direction. No magnitude in this list is a measured constant
/// of this synth.
pub const DIRECTIONS: &[&str] = &[
    "Stevens (1998), Acoustic Phonetics, MIT Press: the velopharyngeal port couples the nasal cavity. Nasal consonants radiate a nasal murmur. No pole frequency is copied.",
    "House and Stevens (1956), JASA 28(3):303-310: opening the velum adds low-frequency energy and an antiresonance in the oral transfer. No hertz is copied.",
    "Chen (1997), JASA 102(4):2360-2370: A1-P0 rises with vowel nasalization. The dB per unit of this velum control is not in that paper.",
    "Bachorowski, Smoski and Owren (2001), JASA 110(3):1581-1597, Tables I and II: laugh call duration, calls per bout, and open-mouth versus closed-mouth F0 by sex. Absolute laugh Hz, not z against speech.",
    "Cowen, Elfenbein, Laukka and Keltner (2019), American Psychologist 74(6):698-712, Figure 3B: 24 categories. A laugh is amusement with no words. A sigh is not given one category.",
    "ARTICULATION-V2-SPEC §2: no per-region nasalance was read. Nasality stays out of the labels. phon_nasal is a speech-class posterior, not nasal resonance.",
];

/// Where the neighbouring code comes from. This crate does not reimplement it.
pub const REUSES: &[&str] = &[
    "Burst names and the laughter worker JSON: tools/symphony/articulation (draft PR 191, state::resolve and workers/laughter_worker.py). measure and detect stay in that crate.",
    "Decode burst track JSON: tools/symphony/articulation/src/decode.rs BurstTrack and TickState.burst, same PR.",
    "F0 band cut 165 Hz: speaker_baseline in that crate. A prior for the label column, not a new sexing.",
    "Emotion inventory: configs/emotions-v2.toml (draft PR 189). This crate does not edit the list.",
    "Noise LCG: crates/galion-heartbeat/src/presence.rs, seed 13. Used for unvoiced bursts, not as a breath model.",
    "Intent document: galion-mne-contract (draft PR 198), via contract::request_from_document.",
];
