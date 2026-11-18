// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Speaker baseline for this layer. Sex, age and tract length sit here, not
//! in the feeling. Resting velum is this speaker's own neutral (z = 0).
//! `phon_nasal` is stored when a decode profile has it, and is not a velum.

use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    /// Decoder column for F0 below the 165 Hz cut. Laugh F0 uses the men's means.
    Low,
    /// Decoder column for F0 at or above that cut. Laugh F0 uses the women's means.
    High,
    /// No baseline F0. Neither sex mean is used.
    Unset,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Speaker {
    #[serde(default)]
    pub f0_hz: f64,
    /// 0 closed, 1 coupled. The caller's own-neutral velum, not a population nasalance.
    #[serde(default)]
    pub velum_rest: f64,
    /// Phonet nasal-class posterior, if a card carried one. Not nasalance.
    #[serde(default)]
    pub phon_nasal: Option<f64>,
}

impl Default for Speaker {
    fn default() -> Self {
        Speaker {
            f0_hz: 0.0,
            velum_rest: 0.0,
            phon_nasal: None,
        }
    }
}

impl Speaker {
    pub fn check(&self) -> Result<(), String> {
        if !self.f0_hz.is_finite() || self.f0_hz < 0.0 {
            return Err("speaker.f0_hz must be finite and >= 0".into());
        }
        if !self.velum_rest.is_finite() || !(0.0..=1.0).contains(&self.velum_rest) {
            return Err("speaker.velum_rest must be in [0, 1]".into());
        }
        if let Some(p) = self.phon_nasal {
            if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                return Err("speaker.phon_nasal must be in [0, 1] when set".into());
            }
        }
        Ok(())
    }

    pub fn band(&self, cfg: &Config) -> Band {
        if self.f0_hz <= 0.0 {
            Band::Unset
        } else if self.f0_hz >= cfg.speaker.f0_band_hz {
            Band::High
        } else {
            Band::Low
        }
    }
}
