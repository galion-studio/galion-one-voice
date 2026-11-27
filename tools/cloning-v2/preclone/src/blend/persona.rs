// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Aria, a new synthetic voice. Not a person.
//!
//! The owner asked for Samantha's warmth and curiosity with Jarvis's calm
//! precision: playful, honest, emotionally attentive, steady at work, and
//! 23 years old. Honest is how she answers. It does not move a formant.
//! These knobs are that brief. They are not a measurement of anyone.

use super::num::publish;
use super::spec::EmotionDefault;

pub const TARGET_AGE_YEARS: f64 = 23.0;
/// Design anchor for the two adult roles. Not a birthday.
pub(crate) const ANCHOR_AGE_YEARS: f64 = 32.0;
/// Fraction of F0 added per year younger than the anchor.
pub(crate) const F0_SCALE_PER_YEAR_YOUNGER: f64 = 0.0035;
/// Fraction of formant frequency and centroid added per year younger.
pub(crate) const FORMANT_SCALE_PER_YEAR_YOUNGER: f64 = 0.0030;

pub(crate) const WARMTH_HNR_DB: f64 = -0.6;
pub(crate) const PLAYFUL_SPAN_ST: f64 = 0.5;
pub(crate) const STEADY_RATE: f64 = 0.985;

pub(crate) const PERSONA_ID: &str = "aria";
pub(crate) const VOICE_ID: &str = "aria-steady";
pub(crate) const FEEDS: &str = "galion-emotion-core.VoiceBaseline";
pub(crate) const SLOT_BYTES: u64 = 16;
pub(crate) const AXIS_ORDER: &[&str] = &["valence", "arousal", "dominance", "approach"];
pub(crate) const TRAITS: &[&str] = &[
    "playful",
    "honest",
    "emotionally-attentive",
    "steady-at-work",
];
pub(crate) const FROM_SAMANTHA: &[&str] = &["warmth", "curiosity"];
pub(crate) const FROM_JARVIS: &[&str] = &["calm", "precision"];

// Rest anchors. Valence and approach are −1..1. Arousal and dominance are
// 0..1. Same slots as galion-emotion-core `VoiceBaseline::pos`.
const SAM_VALENCE: f64 = 0.34;
const SAM_AROUSAL: f64 = 0.40;
const SAM_DOMINANCE: f64 = 0.38;
const SAM_APPROACH: f64 = 0.30;
const JAR_VALENCE: f64 = 0.10;
const JAR_AROUSAL: f64 = 0.24;
const JAR_DOMINANCE: f64 = 0.60;
const JAR_APPROACH: f64 = 0.08;

const W_VALENCE_SAM: f64 = 0.62;
const W_AROUSAL_SAM: f64 = 0.45;
const PLAYFUL_AROUSAL: f64 = 0.04;
const W_DOMINANCE_SAM: f64 = 0.40;
const STEADY_DOMINANCE: f64 = -0.04;
const W_APPROACH_SAM: f64 = 0.70;

pub(crate) fn emotion_point(pitch_centre_hz: f64) -> Result<EmotionDefault, String> {
    let valence = publish(W_VALENCE_SAM * SAM_VALENCE + (1.0 - W_VALENCE_SAM) * JAR_VALENCE);
    let arousal = publish(
        W_AROUSAL_SAM * SAM_AROUSAL + (1.0 - W_AROUSAL_SAM) * JAR_AROUSAL + PLAYFUL_AROUSAL,
    );
    let dominance = publish(
        W_DOMINANCE_SAM * SAM_DOMINANCE
            + (1.0 - W_DOMINANCE_SAM) * JAR_DOMINANCE
            + STEADY_DOMINANCE,
    );
    let approach = publish(W_APPROACH_SAM * SAM_APPROACH + (1.0 - W_APPROACH_SAM) * JAR_APPROACH);
    in_axis("valence", valence, -1.0, 1.0)?;
    in_axis("arousal", arousal, 0.0, 1.0)?;
    in_axis("dominance", dominance, 0.0, 1.0)?;
    in_axis("approach", approach, -1.0, 1.0)?;
    if !fits_slot(PERSONA_ID) || !fits_slot(VOICE_ID) {
        return Err("refused: aria id does not fit the 16-byte VoiceBaseline slot".into());
    }
    Ok(EmotionDefault {
        persona_id: PERSONA_ID.to_string(),
        voice_id: VOICE_ID.to_string(),
        valence,
        arousal,
        dominance,
        approach,
        pitch_centre_hz,
    })
}

fn in_axis(name: &str, value: f64, lo: f64, hi: f64) -> Result<(), String> {
    if value.is_finite() && (lo..=hi).contains(&value) {
        Ok(())
    } else {
        Err(format!("refused: emotion {name} left {lo}..={hi}"))
    }
}

pub(crate) fn fits_slot(id: &str) -> bool {
    id.len() <= 15 && !id.bytes().any(|b| b == 0)
}
