// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Inside the body: breath, energy, fatigue, health.
//!
//! These are not emotion names. Each coefficient below is a stand-in knob.
//! The lungs layer records fatigue's published magnitude as none, and this
//! crate does not invent a litre or a clinical score. The knobs exist so a
//! closed eval can see the moment move.

use crate::axes::{clamp01, APPROACH, AROUSAL, AXES, DOMINANCE, VALENCE};

/// Energy at rest. Offset is `energy - ENERGY_REST`.
pub const ENERGY_REST: f32 = 0.70;
/// Fatigue at rest. A fresh speaker is not at zero.
pub const FATIGUE_REST: f32 = 0.05;
/// Health at rest. 1 is well. 0 is unwell. Not a diagnosis.
pub const HEALTH_REST: f32 = 1.0;
/// Relative lung volume at rest. Not a litre.
pub const VOLUME_REST: f32 = 0.55;

/// Stand-in breath cycle. 50 frames × 80 ms = 4.00 s.
pub const BREATH_CYCLE_FRAMES: u32 = 50;

pub const FATIGUE_VALENCE: f32 = -0.20;
pub const FATIGUE_AROUSAL: f32 = -0.35;
pub const FATIGUE_DOMINANCE: f32 = -0.10;
pub const FATIGUE_APPROACH: f32 = -0.15;

pub const ENERGY_AROUSAL: f32 = 0.25;
pub const ENERGY_APPROACH: f32 = 0.20;

pub const HEALTH_VALENCE: f32 = 0.18;
pub const HEALTH_AROUSAL: f32 = 0.12;
pub const HEALTH_DOMINANCE: f32 = 0.10;

pub const BREATH_VOLUME_AROUSAL: f32 = 0.30;
pub const BREATH_FLOW_AROUSAL: f32 = 0.08;

/// Physiology stage names line up with `galion-mne-contract` `BreathSegment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BreathPhase {
    Rest = 0,
    Inhale = 1,
    Hold = 2,
    Speech = 3,
    Exhale = 4,
}

impl BreathPhase {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Rest,
            1 => Self::Inhale,
            2 => Self::Hold,
            3 => Self::Speech,
            4 => Self::Exhale,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Inhale => "inhale",
            Self::Hold => "hold",
            Self::Speech => "speech",
            Self::Exhale => "exhale",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "rest" => Self::Rest,
            "inhale" => Self::Inhale,
            "hold" => Self::Hold,
            "speech" => Self::Speech,
            "exhale" => Self::Exhale,
            _ => return None,
        })
    }
}

/// Lungs and air on one tick. Volume is 0..1. Flow is −1 (out) .. +1 (in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    pub phase: BreathPhase,
    pub volume: f32,
    pub flow: f32,
}

impl Breath {
    pub const fn rest() -> Self {
        Self {
            phase: BreathPhase::Rest,
            volume: VOLUME_REST,
            flow: 0.0,
        }
    }
}

/// Body factors the moment chases. Not categories.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inside {
    pub breath: Breath,
    pub energy: f32,
    pub fatigue: f32,
    pub health: f32,
}

impl Inside {
    pub const fn rest() -> Self {
        Self {
            breath: Breath::rest(),
            energy: ENERGY_REST,
            fatigue: FATIGUE_REST,
            health: HEALTH_REST,
        }
    }
}

/// Piecewise stand-in when a tick does not bring its own breath.
/// Frame 0 of the cycle is the start of an inhale.
pub fn standin_breath(frame: u32) -> Breath {
    let p = frame % BREATH_CYCLE_FRAMES;
    if p < 15 {
        let u = p as f32 / 15.0;
        Breath {
            phase: BreathPhase::Inhale,
            volume: clamp01(0.40 + 0.45 * u),
            flow: 0.70,
        }
    } else if p < 20 {
        Breath {
            phase: BreathPhase::Hold,
            volume: 0.85,
            flow: 0.0,
        }
    } else if p < 40 {
        let u = (p - 20) as f32 / 20.0;
        Breath {
            phase: BreathPhase::Exhale,
            volume: clamp01(0.85 - 0.50 * u),
            flow: -0.60,
        }
    } else {
        Breath {
            phase: BreathPhase::Rest,
            volume: 0.35,
            flow: 0.0,
        }
    }
}

/// Added to the baseline to form the spring target. Zero at [`Inside::rest`].
pub fn factor_offset(inside: &Inside) -> [f32; AXES] {
    let energy = inside.energy - ENERGY_REST;
    let fatigue = inside.fatigue - FATIGUE_REST;
    let health = inside.health - HEALTH_REST;
    let volume = inside.breath.volume - VOLUME_REST;
    let flow = inside.breath.flow;
    let mut out = [0.0; AXES];
    out[VALENCE] = FATIGUE_VALENCE * fatigue + HEALTH_VALENCE * health;
    out[AROUSAL] = FATIGUE_AROUSAL * fatigue
        + ENERGY_AROUSAL * energy
        + HEALTH_AROUSAL * health
        + BREATH_VOLUME_AROUSAL * volume
        + BREATH_FLOW_AROUSAL * flow;
    out[DOMINANCE] = FATIGUE_DOMINANCE * fatigue + HEALTH_DOMINANCE * health;
    out[APPROACH] = ENERGY_APPROACH * energy + FATIGUE_APPROACH * fatigue;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_offset_is_zero_and_phase_names_match_the_contract() {
        let off = factor_offset(&Inside::rest());
        for v in off {
            assert_eq!(v.to_bits(), 0.0f32.to_bits());
        }
        assert_eq!(BreathPhase::Rest.as_str(), "rest");
        assert_eq!(BreathPhase::Inhale.as_str(), "inhale");
        assert_eq!(BreathPhase::Hold.as_str(), "hold");
        assert_eq!(BreathPhase::Speech.as_str(), "speech");
        assert_eq!(BreathPhase::Exhale.as_str(), "exhale");
        assert_eq!(standin_breath(0).phase, BreathPhase::Inhale);
        assert_eq!(standin_breath(15).phase, BreathPhase::Hold);
        assert_eq!(standin_breath(20).phase, BreathPhase::Exhale);
        assert_eq!(standin_breath(40).phase, BreathPhase::Rest);
        assert_eq!(BREATH_CYCLE_FRAMES * crate::clock::FRAME_MS, 4_000);
    }
}
