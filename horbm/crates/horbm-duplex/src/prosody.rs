// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Prosody and ear sensing. Energy plus a pitch estimate. No model.
//!
//! The voiced clamp is the orchestrator energy gate: floor −60 dBFS, +12 dB,
//! clamped at −48 dB. One 80 ms frame is four 20 ms slices, so a frame that
//! is loud for the whole frame has already passed the 60 ms onset.
//!
//! Compiled in the default build. [`ProsodyEar::set_enabled`] is the runtime
//! off switch. `--no-default-features` leaves the measurement out.

use crate::ports::{EarSense, ProsodyCue};

/// Voiced above this. Same clamp as the energy gate (−48 dBFS).
#[cfg(feature = "prosody")]
pub const VOICED_DB: f32 = -48.0;

/// Speech band for the pitch search. 50 Hz to 500 Hz at 16 kHz.
#[cfg(feature = "prosody")]
const MIN_LAG: usize = 32;
#[cfg(feature = "prosody")]
const MAX_LAG: usize = 320;

/// Default ear sense. On when the `prosody` feature is compiled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProsodyEar {
    enabled: bool,
}

impl Default for ProsodyEar {
    fn default() -> Self {
        Self::new()
    }
}

impl ProsodyEar {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: cfg!(feature = "prosody"),
        }
    }

    /// The feature is in this build. The runtime switch is separate.
    #[must_use]
    pub const fn compiled() -> bool {
        cfg!(feature = "prosody")
    }
}

impl EarSense for ProsodyEar {
    fn hear(&mut self, pcm: &[i16]) -> ProsodyCue {
        if !self.enabled || !Self::compiled() {
            return ProsodyCue::disabled();
        }
        #[cfg(feature = "prosody")]
        {
            measure(pcm)
        }
        #[cfg(not(feature = "prosody"))]
        {
            let _ = pcm;
            ProsodyCue::disabled()
        }
    }

    fn enabled(&self) -> bool {
        self.enabled && Self::compiled()
    }

    fn set_enabled(&mut self, on: bool) {
        self.enabled = on && Self::compiled();
    }
}

#[cfg(feature = "prosody")]
fn measure(pcm: &[i16]) -> ProsodyCue {
    let rms_db = rms_db(pcm);
    let voiced = rms_db > VOICED_DB;
    let f0_hz = if voiced { fundamental_hz(pcm) } else { 0.0 };
    ProsodyCue {
        enabled: true,
        voiced,
        rms_db,
        f0_hz,
    }
}

/// RMS in dB. Same formula as the orchestrator gate: `10 log10(mean square)`.
#[cfg(feature = "prosody")]
pub fn rms_db(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return -120.0;
    }
    let mut energy = 0.0f64;
    for sample in frame {
        let x = f64::from(*sample) / 32768.0;
        energy += x * x;
    }
    let mean = energy / frame.len() as f64;
    (10.0 * mean.max(1.0e-12).log10()) as f32
}

#[cfg(feature = "prosody")]
fn fundamental_hz(pcm: &[i16]) -> f32 {
    use crate::SAMPLE_RATE;
    if pcm.len() <= MAX_LAG + 1 {
        return 0.0;
    }
    let mut energy = 0.0f64;
    for sample in pcm {
        let x = f64::from(*sample);
        energy += x * x;
    }
    if energy < 1.0 {
        return 0.0;
    }
    let mut corr = [0.0f64; MAX_LAG + 1];
    let mut best = 0.0f64;
    for lag in MIN_LAG..=MAX_LAG {
        let mut sum = 0.0f64;
        let n = pcm.len() - lag;
        for i in 0..n {
            sum += f64::from(pcm[i]) * f64::from(pcm[i + lag]);
        }
        let norm = sum / n as f64;
        corr[lag] = norm;
        if norm > best {
            best = norm;
        }
    }
    let floor = energy / pcm.len() as f64;
    if best < 0.25 * floor {
        return 0.0;
    }
    let keep = best * 0.9;
    let mut chosen = 0usize;
    for lag in (MIN_LAG + 1)..MAX_LAG {
        if corr[lag] >= keep && corr[lag] >= corr[lag - 1] && corr[lag] >= corr[lag + 1] {
            chosen = lag;
            break;
        }
    }
    if chosen == 0 {
        return 0.0;
    }
    SAMPLE_RATE as f32 / chosen as f32
}
