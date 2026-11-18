// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Lungs and air for one utterance.
//!
//! This is the physiology track. It is not one of the 24 Cowen categories and
//! it is not a vocal burst. Emotion, arousal, energy and fatigue are inputs.
//! They change how the breath is planned. They do not become the breath's name.
//!
//! Detection is not reimplemented. A `lungs` object or a cue list from the
//! features agent's breath stage (`galion-articulation` `breath.rs`, agent
//! bc-56b9f22c) is parsed in [`stage`] and, when the caller also passes that
//! speaker's own samples, those samples are the inhale timbre. Otherwise the
//! inhale is the heartbeat's white-noise breath, scaled to the planned level.
//!
//! Directions are cited in `docs/galion-voice-v2/symphony/LUNGS.md`.
//! Magnitudes live in `configs/symphony/lungs.toml` and are not measurements.
#![forbid(unsafe_code)]

pub mod config;
pub mod inventory;
pub mod plan;
pub mod stage;
pub mod synth;
pub mod wav;

/// Symphony heartbeat. LOCKS `clock.frame_ms` = 80.
pub const TICK_S: f64 = 0.08;
pub const TICK_MS: u32 = 80;

/// Papers used for a direction. No magnitude in this list is a measured constant.
pub const DIRECTIONS: &[&str] = &[
    "Hixon, Goldman and Mead (1973), JSHR 16:78-115: speech is exhaled, and lung volume falls across the breath group.",
    "Winkworth, Davis, Adams and Ellis (1994), JSHR 37:535-556, and (1995), JSHR 38:124-144: inhalations sit at linguistic boundaries, and a longer following phrase is preceded by a deeper inspiration.",
    "Whalen and Kinsella-Shaw (1997), Phonetica 54:138-152: inspiration duration increases with the upcoming utterance.",
    "Ladefoged and McKinney (1963), JASA 35:454-460: vocal loudness increases with subglottal pressure.",
    "Titze (1989), JASA 85(2):901-906: fundamental frequency rises with subglottal pressure.",
    "Lieberman (1967), Intonation, perception, and language: falling subglottal pressure across a breath group contributes to F0 declination. Strik and Boves (1995), Journal of Phonetics 23:375-394, measured the F0 and subglottal-pressure downtrend together.",
    "Boiten (1998), Biological Psychology 49:29-51, abstract: amusement shortens inspiratory time and reduces tidal volume; disgust prolongs the inspiratory pause.",
    "Homma and Masaoka (2008), Experimental Physiology 93(9):1011-1021: breathing is faster in an arousal state, and faster in anticipatory anxiety, beyond oxygen demand.",
    "Cowen, Elfenbein, Laukka and Keltner (2019), American Psychologist 74(6):698-712: 24 categories. Breath is not one of them.",
    "Juslin and Laukka (2003), Psychological Bulletin 129(5):770-814: high arousal is louder. Loudness is the proxy of expiratory drive, not a pressure in cm H2O.",
];

/// Where the audible-breath machinery comes from. Not physiology papers.
pub const REUSES: &[&str] = &[
    "Breath cues and splice_inhales: tools/symphony/articulation/src/breath.rs on cursor/articulation-v2-cad4 (features agent bc-56b9f22c, draft PR 191). measure() stays in that crate because it needs the articulation card.",
    "Breath noise: crates/galion-heartbeat/src/presence.rs RikaReplayBackend synth, white noise, RMS 0.003, LCG seed 13.",
    "Breath-versus-silence floor: crates/galion-presence/src/wav.rs Gate::default breath_dbfs -55.",
];
