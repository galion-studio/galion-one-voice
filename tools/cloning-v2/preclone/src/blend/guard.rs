// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Identity-distance guard.
//!
//! Two closed checks. The source mark is `own-voice` or
//! `fictional-reference`, and the published Aria baseline sits at least
//! [`MIN_BASELINE_DISTANCE`] from each parent on the normalised features.
//!
//! A real third-party mark is refused even when the numbers would blend.
//! This stage does not listen, so it cannot recognise a person. The mark
//! is the operator's declaration. The distance check stops a blend that
//! lands on a parent, including a parent copied into both slots: the age
//! and trait move alone stays under the minimum.

use super::num::{fmt, publish};
use super::spec::{BaselineTarget, SourceConsent};
use crate::profile::VoiceProfile;

/// Euclidean distance, in units of [`SCALES`]. At least this far from each parent.
pub const MIN_BASELINE_DISTANCE: f64 = 1.25;

pub(crate) const DISTANCE_METRIC: &str = "euclidean-normalised-baseline";

/// One step that counts as a different voice on that feature.
pub(crate) const SCALES: [(&str, f64); 9] = [
    ("f0_hz", 25.0),
    ("f0_span_hz", 12.0),
    ("f1_hz", 60.0),
    ("f2_hz", 120.0),
    ("f3_hz", 150.0),
    ("spectral_centroid_hz", 150.0),
    ("rate", 0.05),
    ("breathiness", 0.12),
    ("hnr_db", 3.0),
];

const REAL_THIRD_PARTY: &[&str] = &[
    "third-party",
    "real-person",
    "real-third-party",
    "celebrity",
    "donor",
    "cc0-donor",
    "performer",
    "film-audio",
];

pub(crate) fn consent_from(text: &str) -> Result<SourceConsent, String> {
    match text {
        "own-voice" => Ok(SourceConsent::OwnVoice),
        "fictional-reference" => Ok(SourceConsent::FictionalReference),
        other if REAL_THIRD_PARTY.contains(&other) => {
            Err("refused: real third-party voices are never allowed".into())
        }
        other => Err(format!(
            "refused: consent \"{}\" is not an allowed source",
            escape_msg(other)
        )),
    }
}

pub(crate) fn check_voice(slot: &str, v: &VoiceProfile) -> Result<(), String> {
    finite(slot, "f0_hz", v.f0_hz)?;
    finite(slot, "f1_hz", v.f1_hz)?;
    finite(slot, "f2_hz", v.f2_hz)?;
    finite(slot, "f3_hz", v.f3_hz)?;
    finite(slot, "spectral_centroid_hz", v.spectral_centroid_hz)?;
    finite(slot, "hnr_db", v.hnr_db)?;
    finite(slot, "voiced_fraction", v.voiced_fraction)?;
    if !(70.0..=400.0).contains(&v.f0_hz) {
        return Err(format!("refused: {slot} f0_hz is outside 70..=400"));
    }
    let formants_ok = (90.0..=1200.0).contains(&v.f1_hz)
        && (200.0..=3000.0).contains(&v.f2_hz)
        && (800.0..=5000.0).contains(&v.f3_hz)
        && v.f1_hz < v.f2_hz
        && v.f2_hz < v.f3_hz;
    if !formants_ok {
        return Err(format!(
            "refused: {slot} formants are outside the design band or out of order"
        ));
    }
    if !(100.0..=8000.0).contains(&v.spectral_centroid_hz) {
        return Err(format!(
            "refused: {slot} spectral_centroid_hz is outside 100..=8000"
        ));
    }
    if !(-5.0..=40.0).contains(&v.hnr_db) {
        return Err(format!("refused: {slot} hnr_db is outside -5..=40"));
    }
    if !(0.0..=1.0).contains(&v.voiced_fraction) {
        return Err(format!("refused: {slot} voiced_fraction is outside 0..=1"));
    }
    Ok(())
}

pub(crate) fn accept(
    slot: &str,
    raw: BaselineTarget,
    child: bool,
) -> Result<BaselineTarget, String> {
    let b = publish_baseline(raw);
    if !(70.0..=400.0).contains(&b.f0_centre_hz) {
        return Err(format!("refused: {slot} f0_centre_hz left 70..=400"));
    }
    if child && (!(70.0..=400.0).contains(&b.f0_lo_hz) || !(70.0..=400.0).contains(&b.f0_hi_hz)) {
        return Err(format!("refused: {slot} f0 range left 70..=400"));
    }
    if !(b.f0_lo_hz < b.f0_centre_hz && b.f0_centre_hz < b.f0_hi_hz) {
        return Err(format!(
            "refused: {slot} f0 range does not contain its centre"
        ));
    }
    if !(b.f1_hz < b.f2_hz && b.f2_hz < b.f3_hz) {
        return Err(format!("refused: {slot} formants are out of order"));
    }
    if !(0.0..=1.0).contains(&b.breathiness) {
        return Err(format!("refused: {slot} breathiness left 0..=1"));
    }
    if !(0.80..=1.20).contains(&b.rate) {
        return Err(format!("refused: {slot} rate left 0.80..=1.20"));
    }
    if !b.f0_span_hz.is_finite() || b.f0_span_hz <= 0.0 {
        return Err(format!("refused: {slot} f0 span is not positive"));
    }
    Ok(b)
}

pub(crate) fn require_distance(to_samantha: f64, to_jarvis: f64) -> Result<(), String> {
    let mut problems = Vec::new();
    if too_close(to_samantha) {
        problems.push(format!(
            "identity distance to samantha-slot is {} (minimum {})",
            fmt(to_samantha),
            fmt(MIN_BASELINE_DISTANCE)
        ));
    }
    if too_close(to_jarvis) {
        problems.push(format!(
            "identity distance to jarvis-slot is {} (minimum {})",
            fmt(to_jarvis),
            fmt(MIN_BASELINE_DISTANCE)
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("refused: {}", problems.join("; ")))
    }
}

fn too_close(distance: f64) -> bool {
    !distance.is_finite() || distance < MIN_BASELINE_DISTANCE
}

pub fn baseline_distance(a: &BaselineTarget, b: &BaselineTarget) -> f64 {
    let da = features(a);
    let db = features(b);
    let mut sum = 0.0;
    for i in 0..SCALES.len() {
        let d = (da[i] - db[i]) / SCALES[i].1;
        sum += d * d;
    }
    sum.sqrt()
}

fn features(b: &BaselineTarget) -> [f64; 9] {
    [
        b.f0_centre_hz,
        b.f0_span_hz,
        b.f1_hz,
        b.f2_hz,
        b.f3_hz,
        b.spectral_centroid_hz,
        b.rate,
        b.breathiness,
        b.hnr_db,
    ]
}

fn publish_baseline(b: BaselineTarget) -> BaselineTarget {
    BaselineTarget {
        f0_centre_hz: publish(b.f0_centre_hz),
        f0_lo_hz: publish(b.f0_lo_hz),
        f0_hi_hz: publish(b.f0_hi_hz),
        f0_span_hz: publish(b.f0_span_hz),
        f1_hz: publish(b.f1_hz),
        f2_hz: publish(b.f2_hz),
        f3_hz: publish(b.f3_hz),
        spectral_centroid_hz: publish(b.spectral_centroid_hz),
        timbre: b.timbre,
        rate: publish(b.rate),
        breathiness: publish(b.breathiness),
        hnr_db: publish(b.hnr_db),
        voiced_fraction: publish(b.voiced_fraction),
    }
}

fn finite(slot: &str, name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("refused: {slot} {name} is not finite"))
    }
}

fn escape_msg(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars().take(80) {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() => o.push(' '),
            c => o.push(c),
        }
    }
    o
}
