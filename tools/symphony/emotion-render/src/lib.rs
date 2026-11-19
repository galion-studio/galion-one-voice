// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Render side of the fluid emotion core.
//!
//! A per-80 ms target (any of the 24 Cowen 2019 categories, on prosody or on
//! burst, plus arousal, valence and dominance) becomes controls for the
//! existing cloning layer. Directions are cited in [`direction`]. Magnitudes
//! are [`scales`], and an unknown magnitude does not become a number.

mod direction;
mod error;
mod inventory;
mod render;
mod scales;

pub use direction::{speech_directions, Sign, COWEN_AP, COWEN_NHB};
pub use error::RenderError;
pub use inventory::{
    check_inventory_file, resolve_name, Category, ProsodyTier, Resolved, Track, CATEGORIES, TICK_MS,
};
pub use render::{
    clone_request_patch, render_tick, render_utterance, render_word, Applied, Avd, ChatterboxKnobs,
    Directions, ProsodyKnobs, TargetTick, TickControls, UtteranceControls, WordRender, SCHEMA,
};
pub use scales::{load_scales, parse_scales, EngineDefaults, FeatureScales, Scales};
