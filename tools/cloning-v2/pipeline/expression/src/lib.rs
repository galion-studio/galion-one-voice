//! Expression modelling for Galion Voice cloning, branch B.
//!
//! The analysis reads a reference (or a CC0 stand-in) and records F0, energy,
//! spectral tilt, singing/vibrato, and any emotion2vec+ or CLAP scores the
//! caller supplies. The plan carries that profile. It does not pick reference
//! clips. Love/warmth conditioning is opt-in and Samantha-only. Jarvis's
//! judged set stays neutral and logic. Crying and yelling are excluded.

pub mod love;
pub mod plan;
pub mod profile;
pub mod prosody;
pub mod wav;

pub use love::{diagnose, diagnose_gate_document, attribute_love, emotion_match, GATE};
pub use plan::{build_plan, PlanRequest};
pub use profile::{analyze_channels, ExpressionProfile};
pub use prosody::analyze_prosody;
