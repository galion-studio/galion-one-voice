// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Equal-weight blend, then the age shift, then Aria's small trait offsets.
//!
//! CPU arithmetic only. The same two profiles always take the same path.

use crate::profile::VoiceProfile;

use super::spec::{BaselineTarget, TimbreKind};

pub(crate) const ACOUSTIC_WEIGHT: f64 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Raw {
    pub f0: f64,
    pub f1: f64,
    pub f2: f64,
    pub f3: f64,
    pub centroid: f64,
    pub hnr: f64,
    pub voiced: f64,
}

impl Raw {
    pub(crate) fn from_voice(v: &VoiceProfile) -> Self {
        Self {
            f0: v.f0_hz,
            f1: v.f1_hz,
            f2: v.f2_hz,
            f3: v.f3_hz,
            centroid: v.spectral_centroid_hz,
            hnr: v.hnr_db,
            voiced: v.voiced_fraction,
        }
    }
}

pub(crate) fn mix(a: Raw, b: Raw) -> Raw {
    let w = ACOUSTIC_WEIGHT;
    Raw {
        f0: w * a.f0 + (1.0 - w) * b.f0,
        f1: w * a.f1 + (1.0 - w) * b.f1,
        f2: w * a.f2 + (1.0 - w) * b.f2,
        f3: w * a.f3 + (1.0 - w) * b.f3,
        centroid: w * a.centroid + (1.0 - w) * b.centroid,
        hnr: w * a.hnr + (1.0 - w) * b.hnr,
        voiced: w * a.voiced + (1.0 - w) * b.voiced,
    }
}

pub(crate) fn shift_age(r: Raw, f0_scale: f64, formant_scale: f64) -> Raw {
    Raw {
        f0: r.f0 * f0_scale,
        f1: r.f1 * formant_scale,
        f2: r.f2 * formant_scale,
        f3: r.f3 * formant_scale,
        centroid: r.centroid * formant_scale,
        hnr: r.hnr,
        voiced: r.voiced,
    }
}

pub(crate) fn apply_warmth(mut r: Raw, hnr_db: f64) -> Raw {
    r.hnr += hnr_db;
    r
}

/// Breath from harmonicity: 24 dB HNR is clear, 0 dB is fully breathy.
fn breathiness(hnr_db: f64) -> f64 {
    ((24.0 - hnr_db) / 24.0).clamp(0.0, 1.0)
}

/// Relative design rate. Higher F0 reads slightly quicker. Not a syllable count.
fn rate_of(f0: f64, voiced: f64, scale: f64) -> f64 {
    let f0_term = (f0 - 150.0) / 2000.0;
    let voice_term = (voiced - 0.70) * 0.10;
    ((1.0 + f0_term + voice_term) * scale).clamp(0.80, 1.20)
}

/// Full F0 width in semitones. Clearer speech is narrower. `bonus` is Aria's play.
fn span_st(hnr_db: f64, bonus: f64) -> f64 {
    let clear = (hnr_db / 24.0).clamp(0.0, 1.0);
    (3.0 + 3.0 * (1.0 - clear) + bonus).clamp(2.0, 12.0)
}

fn lo_hi(f0: f64, span_st: f64) -> (f64, f64, f64) {
    let half = span_st / 2.0;
    let ratio = (half / 12.0).exp2();
    let lo = f0 / ratio;
    let hi = f0 * ratio;
    (lo, hi, hi - lo)
}

pub(crate) fn derive(
    r: Raw,
    span_bonus: f64,
    rate_scale: f64,
    timbre: TimbreKind,
) -> BaselineTarget {
    let hnr = r.hnr.clamp(-5.0, 40.0);
    let voiced = r.voiced.clamp(0.0, 1.0);
    let (lo, hi, span) = lo_hi(r.f0, span_st(hnr, span_bonus));
    BaselineTarget {
        f0_centre_hz: r.f0,
        f0_lo_hz: lo,
        f0_hi_hz: hi,
        f0_span_hz: span,
        f1_hz: r.f1,
        f2_hz: r.f2,
        f3_hz: r.f3,
        spectral_centroid_hz: r.centroid,
        timbre,
        rate: rate_of(r.f0, voiced, rate_scale),
        breathiness: breathiness(hnr),
        hnr_db: hnr,
        voiced_fraction: voiced,
    }
}
