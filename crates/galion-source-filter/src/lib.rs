// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Source-filter stage for Samantha's mouth.
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
//! dips, and exhale noise. It does not compute glottal open quotient,
//! breathiness, or mouth openness. Those numbers belong to `horbm-breath`
//! (PR 278). [`MouthControls::from_moment`] only turns an exhale into noise
//! and copies a caller velum. It does not write the baseline or the moment.
//! Neutral controls leave every sample alone.
//!
//! Samantha is the only persona the speech stack will run this on.
//! [`applies_to`] is that check. The filter itself does not read the
//! baseline's persona bytes: the emotion-core stand-in is not named
//! Samantha, and the product persona is the stack's.

#![forbid(unsafe_code)]

mod controls;
mod cost;
mod filter;

pub use controls::{
    applies_to, formant_q, frame_samples, tilt_db, MouthControls, TractBaseline, BREATHINESS_DB,
    BREATH_CENTER_HZ, BREATH_FLOOR, F1_Q0, F1_SHIFT, F2_Q0, F2_SHIFT, FORMANT_DB, FRAME_MS,
    NASAL_DEPTH_DB, NASAL_HI_HZ, NASAL_LO_HZ, NEUTRAL_TOLERANCE, OPEN_Q_SCALE, PERSONA, TENSION_DB,
    TILT_PIVOT_HZ,
};
pub use cost::{measure_frame_cost, FrameReport};
pub use filter::{SourceFilter, FRAME_ALLOC_BUDGET};
pub use galion_emotion_core::{Breath, BreathPhase, MomentState, VoiceBaseline};

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(FRAME_MS == galion_emotion_core::FRAME_MS);
