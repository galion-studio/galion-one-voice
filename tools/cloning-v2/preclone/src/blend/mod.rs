// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Aria blend stage.
//!
//! `math` blends two steady baselines. `persona` is the owner's trait mix
//! and the age target. `guard` refuses a source that is not allowed, and
//! refuses a result that sits too close to either parent. `spec` writes
//! `galion-voice-design/1`.
//!
//! CPU arithmetic only. No audio, no model, no crate dependency on
//! `galion-emotion-core`. The emotion block is a value that core can copy
//! into `VoiceBaseline`.

mod guard;
mod math;
mod num;
mod persona;
mod spec;

pub use guard::{baseline_distance, MIN_BASELINE_DISTANCE};
pub use persona::TARGET_AGE_YEARS;
pub use spec::{
    parse_source, parse_spec, source_bytes, spec_bytes, AgeTarget, BaselineTarget, DesignSpec,
    EmotionDefault, ParentSlot, SourceConsent, SourceProfile, TimbreKind, SCHEMA as DESIGN_SCHEMA,
    STAGE as DESIGN_STAGE,
};

use math::Raw;
use persona::{
    ANCHOR_AGE_YEARS, F0_SCALE_PER_YEAR_YOUNGER, FORMANT_SCALE_PER_YEAR_YOUNGER, PLAYFUL_SPAN_ST,
    STEADY_RATE, WARMTH_HNR_DB,
};
use spec::TimbreKind as Kind;

/// Blend the warmth slot and the calm slot into Aria.
///
/// The first profile is the Samantha role (warmth, curiosity). The second
/// is the Jarvis role (calm, precision). Acoustic weight is equal. The
/// persona and the age target are applied to the child after that mix.
pub fn design_aria(warmth: &SourceProfile, calm: &SourceProfile) -> Result<DesignSpec, String> {
    guard::check_voice("samantha-slot", &warmth.voice)?;
    guard::check_voice("jarvis-slot", &calm.voice)?;
    let sam = Raw::from_voice(&warmth.voice);
    let jar = Raw::from_voice(&calm.voice);
    let sam_base = guard::accept(
        "samantha-slot",
        math::derive(sam, 0.0, 1.0, Kind::ParentSteady),
        false,
    )?;
    let jar_base = guard::accept(
        "jarvis-slot",
        math::derive(jar, 0.0, 1.0, Kind::ParentSteady),
        false,
    )?;
    let younger = ANCHOR_AGE_YEARS - TARGET_AGE_YEARS;
    let f0_scale = num::publish(1.0 + younger * F0_SCALE_PER_YEAR_YOUNGER);
    let formant_scale = num::publish(1.0 + younger * FORMANT_SCALE_PER_YEAR_YOUNGER);
    let aged = math::shift_age(math::mix(sam, jar), f0_scale, formant_scale);
    let warmed = math::apply_warmth(aged, WARMTH_HNR_DB);
    let child = guard::accept(
        "aria",
        math::derive(warmed, PLAYFUL_SPAN_ST, STEADY_RATE, Kind::BlendedFormant),
        true,
    )?;
    let distance_samantha = num::publish(baseline_distance(&child, &sam_base));
    let distance_jarvis = num::publish(baseline_distance(&child, &jar_base));
    guard::require_distance(distance_samantha, distance_jarvis)?;
    let emotion = persona::emotion_point(child.f0_centre_hz)?;
    Ok(DesignSpec {
        age: AgeTarget {
            target_years: TARGET_AGE_YEARS,
            anchor_years: ANCHOR_AGE_YEARS,
            shift_years: num::publish(TARGET_AGE_YEARS - ANCHOR_AGE_YEARS),
            f0_scale,
            formant_scale,
        },
        baseline: child,
        emotion,
        samantha: ParentSlot {
            consent: warmth.consent,
            baseline: sam_base,
        },
        jarvis: ParentSlot {
            consent: calm.consent,
            baseline: jar_base,
        },
        distance_samantha,
        distance_jarvis,
    })
}

/// Read two `galion-preclone/1` documents and design Aria.
///
/// Consent is checked here, before any blend. A missing mark, or a real
/// third-party mark, refuses the run.
pub fn design_from_profiles(warmth_json: &str, calm_json: &str) -> Result<DesignSpec, String> {
    let warmth = parse_source(warmth_json).map_err(|e| format!("samantha-slot: {e}"))?;
    let calm = parse_source(calm_json).map_err(|e| format!("jarvis-slot: {e}"))?;
    design_aria(&warmth, &calm)
}
