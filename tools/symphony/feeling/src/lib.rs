// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Feeling core (MnE S2).
//!
//! The state is a point in an open continuous space: valence, arousal,
//! dominance, approach, and any further learned axis a frame names. It is
//! not a category. The 24 Cowen names are optional pins placed, where a
//! direction was read, at the pole of an axis. A pin the intent does not
//! name does not become the category sent downstream.
//!
//! One call of [`Feeling::run`] advances one 80 ms tick per intent frame.
//! The moment chases a blend of the held goal and a slower mood. The mood
//! decays because it chases the moment, and the moment returns to the
//! baseline when the goal does. Half-lives are config. They are not the
//! time constants estimated by Kuppens, Oravecz and Tuerlinckx (2010).
//!
//! Output `controls` is the tick PR #192 (`galion-emotion-render`) reads.
//! Output `lungs` is the moment PR #195 plans a breath from. This crate
//! does not render controls and does not plan breaths.
#![forbid(unsafe_code)]

mod config;
mod feed;
mod inventory;
mod state;

pub use config::Config;
pub use feed::{
    BlendEntry, ControlsTick, FeelingOut, LungsMoment, LungsRequest, NamedValue, NearestPin,
    PinView, Run,
};
pub use inventory::Inventory;
pub use state::{alpha, Feeling};

pub use galion_mne_contract::{IntentFrame, TICK_MS};

pub const SCHEMA: &str = "galion-feeling/1";

/// Core axes, in readout order. Further names are learned and sort after these.
pub const CORE_AXES: [&str; 4] = ["valence", "arousal", "dominance", "approach"];

/// Papers used for a direction. No half-life in this list is a measured constant.
pub const DIRECTIONS: &[&str] = &[
    "Russell (1980), Journal of Personality and Social Psychology 39(6):1161-1178: affect is a continuous pleasure-arousal space. Names are regions in it.",
    "Russell and Mehrabian (1977), Journal of Research in Personality 11:273-294: dominance is a third axis. No per-category loading is copied from that paper.",
    "Carver and Harmon-Jones (2009), Psychological Bulletin 135(2):183-204: anger is an approach affect even with negative valence. Fear is the withdrawal contrast.",
    "Juslin and Laukka (2003), Psychological Bulletin 129(5):770-814, Table 7: anger is fast, loud, and high in F0; sadness is slow, quiet, and low in F0; love-tenderness is slow and quiet. Directions only.",
    "Cowen, Elfenbein, Laukka and Keltner (2019), American Psychologist 74(6):698-712, Figure 3B: 24 names. Optional pins. Coordinates are not copied.",
    "Cowen, Laukka, Elfenbein, Liu and Keltner (2019), Nature Human Behaviour 3(4):369-382: some names form gradients. The 12 reliable prosody names are not a gate.",
    "Kuppens, Oravecz and Tuerlinckx (2010), Journal of Personality and Social Psychology 99(6):1042-1060: core affect has inertia and a home base. Their time constants are not copied.",
    "Thayer (1989), The biopsychology of mood and arousal: energetic arousal is not tense arousal. Energy stays a separate inner factor.",
    "Spec §1.3: fatigue, listener distance, and room have no published feeling magnitude. Those couplings stay unknown and are not applied.",
    "LOCKS clock.frame_ms = 80. The symphony clock, not a feeling time constant.",
];

#[derive(Debug)]
pub struct FeelingError(pub String);

impl std::fmt::Display for FeelingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FeelingError {}
