//! # galion-emotion
//!
//! Emotion as a **duplex channel**: a small, quantized affect band that rides
//! alongside the audio band in both directions.
//!
//! * Inbound  (caller -> Galion): [`detect::AffectDetector`] turns caller PCM into [`EmotionFrame`]s.
//! * Outbound (Galion -> caller): [`express::ProsodyExpressor`] turns a target [`EmotionFrame`]
//!   into [`express::ProsodyControl`] for the TTS / speech decoder.
//! * Every inbound inference passes through [`consent::ConsentGate`] first
//!   (default = deny; GDPR / EU AI Act / Croatian law aware).
//! * [`store::AffectStore`] is the typestate home of consented inbound affect
//!   (default-deny, wipe-on-withdraw hooks); [`film`] is the FiLM conditioning
//!   interface, the counterbalance-not-mirror check and the 3 % budget helper.
//! * [`loudness`] is the estimator seam: [`loudness::GatedDetector`] wraps any
//!   [`loudness::AffectEstimator`] (loudness heuristic, synthetic fixtures, offline
//!   reference replay; later a learned head) behind the same consent gate.
//! * [`band`] defines the 8-byte wire sample and the duplex packet that carries it.
//!
//! Phase 2 status: all model-backed parts are **stubs** (cheap DSP heuristics),
//! zero dependencies, zero model downloads, safetensors + .gq only.

pub mod band;
pub mod consent;
pub mod detect;
pub mod express;
pub mod film;
pub mod frame;
pub mod loudness;
pub mod rika;
pub mod store;

pub use band::{consent_bits, consent_bits_of, encode_with_consent, Direction, DuplexPacket, EmotionBand, EmotionWire};
pub use consent::{ConsentDecision, ConsentGate, ConsentState, DefaultConsentGate, Purpose};
pub use detect::{AffectDetector, DetectError, EnergyStubDetector};
pub use express::{
    empathy_weight, EmpathyPolicy, Oklch, ProsodyControl, ProsodyExpressor, StubExpressor, VoiceColor, EMPATHY_OKLCH,
    EMPATHY_PROSODY, EMPATHY_WASH_AMBER, NEUTRAL_OKLCH,
};
pub use film::{
    apply_film, check_counterbalance, feeling_budget, measure_feeling, within_budget, BudgetReport, FilmConditioner,
    FilmParams, PolicyViolation, StubFilm, FEELING_BUDGET_PCT, FRAME,
};
pub use frame::{Affect, EmotionFrame};
pub use loudness::{
    parse_reference_track, AffectEstimator, Estimate, EstimatorKind, GatedDetector, LoudnessEstimator, RefEntry, RefLabel,
    ReferenceEstimator, SyntheticEstimator, SyntheticMode,
};
pub use rika::{Cue, FrameFeatures, OffReason, Rika, RikaConfig, RikaCtx, RikaOutput, RikaReport, RikaState, Trigger};
pub use store::{AffectBaseline, AffectStore, Consented, Revalidated, Unconsented, WipeHook, WipeReason, WipeReceipt};
