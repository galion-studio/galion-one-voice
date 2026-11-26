// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Source-filter stage for the configured persona's mouth.
//!
//! Piper or flite has already made the waveform. This stage does not
//! synthesise speech. On each 80 ms frame it shapes that waveform and then
//! the caller appends it to the playback buffer.
//!
//! The chain is the one in the articulation spec, applied as filters:
//! glottal tilt, then the oral formants, then two nasal antiresonances,
//! then breath noise on an exhale. The biquad crate owns the filters.
//! Coefficients are the Audio EQ Cookbook forms that crate already ships.
//!
//! [`TractBaseline`] is the steady voice (where F1 and F2 sit at rest).
//! [`MouthControls`] is what this stage renders: tilt, formant shift, nasal
//! dips, and exhale noise. Glottal open quotient, breathiness, nasal
//! coupling, and mouth openness are read from [`horbm_breath::Frame`].
//! This crate does not compute a second Fant model. Neutral controls leave
//! every sample alone.
//!
//! The speech stack runs this stage only when the id matches the configured
//! persona. [`applies_to`] is that exact check. The filter itself does not
//! read the baseline's persona bytes.

#![forbid(unsafe_code)]

mod controls;
mod cost;
mod filter;

pub use controls::{
    applies_to, formant_q, frame_samples, tilt_db, MouthControls, TractBaseline, BREATHINESS_DB,
    BREATH_CENTER_HZ, BREATH_FLOOR, F1_Q0, F1_SHIFT, F2_Q0, F2_SHIFT, FORMANT_DB, FRAME_MS,
    NASAL_DEPTH_DB, NASAL_HI_HZ, NASAL_LO_HZ, NEUTRAL_TOLERANCE, OPEN_Q_SCALE, TENSION_DB,
    TILT_PIVOT_HZ,
};
pub use cost::{measure_frame_cost, FrameReport};
pub use filter::{SourceFilter, FRAME_ALLOC_BUDGET};
pub use galion_emotion_core::{Breath, BreathPhase, MomentState, VoiceBaseline};
pub use horbm_breath::{Articulation, Drive, Frame};

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(FRAME_MS == galion_emotion_core::FRAME_MS);
