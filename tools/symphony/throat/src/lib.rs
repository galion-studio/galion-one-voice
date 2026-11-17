// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Throat / glottal source for one utterance.
//!
//! On each 80 ms tick this layer writes the controls a cloner adapter can
//! apply: pitch relative to the speaker's own baseline, a phonation label
//! (breathy, modal, pressed, creaky), open quotient, spectral tilt, and
//! jitter / shimmer. Feeling supplies the sign. Subglottal pressure supplies
//! the lungs layer's F0 offset and loudness offset. An unknown magnitude is
//! not applied.
//!
//! The pulse is the Liljencrants–Fant model (Fant, Liljencrants and Lin,
//! 1985) reduced to Fant's Rd parameter (Fant, 1995; Fant and Kruckenberg,
//! 1996). It is not a new glottal model. The optional reference is that
//! derivative, for tests. It is not a radiated vowel and not a person.
//!
//! Names come from `galion-mne-contract`. [`contract::drive_from_document`]
//! is the intent path.
#![forbid(unsafe_code)]

pub mod config;
pub mod contract;
pub mod direction;
pub mod inventory;
pub mod lf;
pub mod lungs_in;
pub mod render;
pub mod source;
pub mod wav;

/// Symphony heartbeat. LOCKS `clock.frame_ms` = 80.
pub const TICK_MS: u32 = 80;
pub const TICK_S: f64 = 0.08;

pub const SCHEMA: &str = "galion-throat/1";

/// Papers used for a direction or for the LF model itself.
/// No entry here is an emotion magnitude in semitones or in Rd.
pub const DIRECTIONS: &[&str] = &[
    "Fant, Liljencrants and Lin (1985), STL-QPSR 26(4):1-13: the four-parameter glottal-flow derivative.",
    "Fant (1995), STL-QPSR 36(2-3):119-156: Rd, and the regressions Ra = (-1 + 4.8 Rd) / 100, Rk = (22.4 + 11.8 Rd) / 100, with Rg from the Rd approximation.",
    "Fant and Kruckenberg (1996), TMH-QPSR 37(4):45-56: Rd table; OQ = (1 + Rk) / (2 Rg), the Ra term usually dropped; Fa = F0 / (2 pi Ra); pressed at low Rd, breathy at high Rd; voice effort raises Ee and typically lowers Rd.",
    "Titze (1989), JASA 85(2):901-906: F0 rises with subglottal pressure. The semitone offset is the lungs layer's f0_st, passed through, not recomputed.",
    "Ladefoged and McKinney (1963), JASA 35:454-460: vocal loudness rises with subglottal pressure. Ee follows the lungs layer's loudness_db.",
    "Juslin and Laukka (2003), Psychological Bulletin 129(5), Table 7, pp. 792-795, and p. 796: pitch and high-frequency-energy signs. Jitter has no definitive conclusion there.",
    "Banse and Scherer (1996), JPSP 70(3), Table 6, p. 625: sign only. Those z are not semitones and not Rd.",
    "Hughes, Farley and Rhodes (2010), Journal of Nonverbal Behavior 34(3), p. 160: desire's pitch sign. The published hertz means are not a semitone offset.",
    "Cowen, Elfenbein, Laukka and Keltner (2019), American Psychologist 74(6):698-712: the 24 categories. Breath is not one of them.",
    "Gobl (2017), Interspeech: the Rd regressions are the stated range 0.3 to 2.7.",
];

/// Where the neighbouring numbers come from. This crate does not recompute them.
pub const REUSES: &[&str] = &[
    "Subglottal pressure, f0_st and loudness_db: tools/symphony/lungs on cursor/lungs-breath-layer-b72f (draft PR 195). This layer reads that JSON. It does not plan breaths.",
    "Feeling signs for pitch: the same published calls as tools/symphony/emotion-render (draft PR 192). Step sizes stay in configs/symphony/throat.toml. Burst does not take the speech signs.",
    "Category names: galion-mne-contract (draft PR 198). The throat still keeps its own Feeling value for the LF step.",
    "80 ms clock: LOCKS clock.frame_ms. Not a published glottal time constant.",
];
