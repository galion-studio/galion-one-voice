// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Fant open quotient, breathiness, nasal coupling, and mouth openness.
//!
//! These ride on the lungs frame. They do not plan a second breath, and they
//! do not change volume, flow, or subglottal pressure. The baseline voice is
//! not an input. [`Drive`] already separated that.
//!
//! Fant and Kruckenberg (1996), TMH-QPSR 37(4): Ra and Rk from Rd, then
//! open quotient `(1 + Rk) / (2 Rg)`. Algebra only. No period is sampled.

use galion_emotion_core::{BreathPhase, ENERGY_REST, FATIGUE_REST, HEALTH_REST};

use crate::Drive;

/// Why an Rd value was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlottisError {
    NonFinite,
    OutOfRange,
}

impl std::fmt::Display for GlottisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite => f.write_str("rd is not finite"),
            Self::OutOfRange => f.write_str("rd is out of range"),
        }
    }
}

impl std::error::Error for GlottisError {}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Shape {
    ra: f64,
    rk: f64,
    open_quotient: f64,
}

fn shape(rd: f64) -> Result<Shape, GlottisError> {
    if !rd.is_finite() {
        return Err(GlottisError::NonFinite);
    }
    if !(0.3..=2.7).contains(&rd) {
        return Err(GlottisError::OutOfRange);
    }
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    if ra <= 0.0 || rk <= 0.0 {
        return Err(GlottisError::OutOfRange);
    }
    let a = 0.5 + 1.2 * rk;
    let denom = 0.44 * rd - 4.0 * ra * a;
    if denom <= 0.0 {
        return Err(GlottisError::OutOfRange);
    }
    let rg = rk * a / denom;
    Ok(Shape {
        ra,
        rk,
        open_quotient: (1.0 + rk) / (2.0 * rg),
    })
}

/// Fant open quotient for an Rd in 0.3..=2.7.
pub fn open_quotient_from_rd(rd: f64) -> Result<f64, GlottisError> {
    Ok(shape(rd)?.open_quotient)
}

/// Glottis, nose, and mouth for one frame. Not a second lungs sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Articulation {
    /// Fant open quotient while speaking. Abduction (folds open) otherwise.
    pub open_quotient: f32,
    /// 1 when the frame is air. A mix of Rd while speaking.
    pub breathiness: f32,
    /// 0 oral, 1 nasal. Off speech this is only the resting leak.
    pub nasal_coupling: f32,
    /// 0 closed, 1 open.
    pub mouth_openness: f32,
}

impl Articulation {
    /// Sixteen little-endian bytes. The same controls are the same bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.open_quotient.to_le_bytes());
        out[4..8].copy_from_slice(&self.breathiness.to_le_bytes());
        out[8..12].copy_from_slice(&self.nasal_coupling.to_le_bytes());
        out[12..16].copy_from_slice(&self.mouth_openness.to_le_bytes());
        out
    }
}

/// Controls for this drive and phase.
///
/// `nasal` and `mouth` are caller targets in 0..=1, used while the phase is
/// speech. Fatigue opens the glottis and closes the mouth a little. Low
/// health does the same to the glottis and reduces nasal coupling. Energy
/// above rest adducts slightly. Non-finite targets become 0.
#[must_use]
pub fn controls(drive: &Drive, phase: BreathPhase, nasal: f32, mouth: f32) -> Articulation {
    let drive = drive.clamped();
    let nasal = unit01(nasal);
    let mouth = unit01(mouth);
    if phase != BreathPhase::Speech {
        return Articulation {
            open_quotient: abducted(phase),
            breathiness: 1.0,
            nasal_coupling: nasal_mix(0.0, drive.health),
            mouth_openness: (0.28 + 0.04 * drive.health).clamp(0.20, 0.40),
        };
    }
    let rd = speech_rd(&drive);
    let open_quotient = open_quotient_from_rd(f64::from(rd))
        .unwrap_or(0.65)
        .clamp(0.35, 0.85) as f32;
    Articulation {
        open_quotient,
        breathiness: breathiness(rd),
        nasal_coupling: nasal_mix(nasal, drive.health),
        mouth_openness: (mouth * (1.0 - 0.20 * fatigue_excess(drive.fatigue))).clamp(0.10, 0.90),
    }
}

fn speech_rd(drive: &Drive) -> f32 {
    let rd =
        1.0 + 0.55 * fatigue_excess(drive.fatigue) + 0.45 * (HEALTH_REST - drive.health).max(0.0)
            - 0.15 * (drive.energy - ENERGY_REST).max(0.0);
    rd.clamp(0.5, 2.7)
}

fn breathiness(rd: f32) -> f32 {
    let t = ((rd - 0.5) / 2.2).clamp(0.0, 1.0);
    0.05 + 0.65 * t
}

fn fatigue_excess(fatigue: f32) -> f32 {
    (fatigue - FATIGUE_REST).clamp(0.0, 1.0)
}

fn nasal_mix(target: f32, health: f32) -> f32 {
    const REST: f32 = 0.02;
    let mixed = REST + (1.0 - REST) * target;
    (mixed * (0.40 + 0.60 * health.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

fn abducted(phase: BreathPhase) -> f32 {
    match phase {
        BreathPhase::Inhale => 1.0,
        BreathPhase::Hold => 0.92,
        BreathPhase::Exhale | BreathPhase::Rest | BreathPhase::Speech => 0.95,
    }
}

fn unit01(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_rd_rows_keep_their_open_quotient() {
        let published = [
            (0.3, 0.44, 25.94, 35.0),
            (0.5, 1.40, 28.3, 47.0),
            (0.7, 2.36, 30.7, 55.5),
            (1.0, 3.80, 34.2, 65.0),
            (1.4, 5.72, 38.9, 73.0),
            (2.0, 8.60, 46.0, 78.0),
            (2.7, 11.96, 54.3, 79.0),
        ];
        for (rd, ra_pct, rk_pct, oq_pct) in published {
            let s = shape(rd).unwrap();
            assert!((s.ra * 100.0 - ra_pct).abs() < 0.05, "Ra {rd}");
            assert!((s.rk * 100.0 - rk_pct).abs() < 0.15, "Rk {rd}");
            assert!(
                (s.open_quotient * 100.0 - oq_pct).abs() < 1.0,
                "OQ {rd} got {}",
                s.open_quotient * 100.0
            );
        }
    }

    #[test]
    fn a_higher_rd_is_a_more_open_glottis() {
        let pressed = open_quotient_from_rd(0.5).unwrap();
        let modal = open_quotient_from_rd(1.0).unwrap();
        let breathy = open_quotient_from_rd(2.7).unwrap();
        assert!(pressed < modal && modal < breathy);
        assert!(open_quotient_from_rd(0.0).is_err());
        assert!(open_quotient_from_rd(f64::NAN).is_err());
    }
}
