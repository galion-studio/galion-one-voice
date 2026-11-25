// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! YIN fundamental on one 80 ms frame.
//!
//! de Cheveigné and Kawahara, "YIN, a fundamental frequency estimator for
//! speech and music" (2002): difference function, cumulative mean
//! normalization, absolute threshold, parabolic interpolation.
//!
//! The 24 kHz frame is pair-averaged to 12 kHz first. Speech fundamentals
//! used here (70–420 Hz) sit under the new Nyquist. No heap, no model.

use crate::clock::{FRAME_SAMPLES, SAMPLE_RATE_HZ};

const ANALYSE_RATE: u32 = 12_000;
const HALF: usize = FRAME_SAMPLES / (SAMPLE_RATE_HZ / ANALYSE_RATE) as usize;
const MAX_LAG: usize = 200;
const F0_MIN_HZ: f32 = 70.0;
const F0_MAX_HZ: f32 = 420.0;
const THRESH: f32 = 0.15;
const UNVOICED: f32 = 0.50;

/// Pitch of one frame. `voiced == false` means the frame has no usable period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    pub hz: f32,
    pub harmonicity: f32,
    pub voiced: bool,
}

impl Pitch {
    pub const SILENT: Pitch = Pitch {
        hz: 0.0,
        harmonicity: 0.0,
        voiced: false,
    };
}

/// Estimate F0. A wrong length or a non-finite sample is unvoiced.
pub fn estimate(frame: &[f32]) -> Pitch {
    if frame.len() != FRAME_SAMPLES || frame.iter().any(|s| !s.is_finite()) {
        return Pitch::SILENT;
    }
    let mut half = [0.0f32; HALF];
    let mut i = 0;
    while i < HALF {
        half[i] = 0.5 * (frame[i * 2] + frame[i * 2 + 1]);
        i += 1;
    }
    yin(&half)
}

fn yin(x: &[f32; HALF]) -> Pitch {
    let tau_min = (ANALYSE_RATE as f32 / F0_MAX_HZ).floor().max(2.0) as usize;
    let tau_max = ((ANALYSE_RATE as f32 / F0_MIN_HZ).ceil() as usize)
        .min(HALF / 2)
        .min(MAX_LAG - 1);
    if tau_max <= tau_min + 2 {
        return Pitch::SILENT;
    }
    let w = HALF - tau_max;
    let mut diff = [0.0f32; MAX_LAG];
    let mut tau = 1usize;
    while tau <= tau_max {
        let mut sum = 0.0f32;
        let mut j = 0usize;
        while j < w {
            let d = x[j] - x[j + tau];
            sum += d * d;
            j += 1;
        }
        diff[tau] = sum;
        tau += 1;
    }

    let mut cm = [0.0f32; MAX_LAG];
    cm[0] = 1.0;
    let mut running = 0.0f32;
    tau = 1;
    while tau <= tau_max {
        running += diff[tau];
        cm[tau] = if running > 1e-12 {
            diff[tau] * (tau as f32) / running
        } else {
            1.0
        };
        tau += 1;
    }

    let mut best_tau = tau_min;
    let mut best = f32::MAX;
    let mut picked = None;
    tau = tau_min;
    while tau <= tau_max {
        if cm[tau] < best {
            best = cm[tau];
            best_tau = tau;
        }
        if picked.is_none() && cm[tau] < THRESH {
            let mut t = tau;
            while t < tau_max && cm[t + 1] < cm[t] {
                t += 1;
            }
            picked = Some(t);
        }
        tau += 1;
    }
    let tau = picked.unwrap_or(best_tau);
    if cm[tau] > UNVOICED {
        return Pitch::SILENT;
    }
    let tau_f = parabolic(&cm, tau, tau_max);
    if tau_f < 2.0 {
        return Pitch::SILENT;
    }
    let hz = ANALYSE_RATE as f32 / tau_f;
    if !(F0_MIN_HZ..=F0_MAX_HZ).contains(&hz) {
        return Pitch::SILENT;
    }
    Pitch {
        hz,
        harmonicity: (1.0 - cm[tau]).clamp(0.0, 1.0),
        voiced: true,
    }
}

fn parabolic(cm: &[f32; MAX_LAG], tau: usize, tau_max: usize) -> f32 {
    if tau == 0 || tau >= tau_max {
        return tau as f32;
    }
    let s0 = cm[tau - 1];
    let s1 = cm[tau];
    let s2 = cm[tau + 1];
    let denom = s0 - 2.0 * s1 + s2;
    let delta = if denom.abs() > 1e-12 {
        (0.5 * (s0 - s2) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    tau as f32 + delta
}

/// RMS and zero-crossing rate. Same left-to-right sum as
/// `galion-emotion` `loudness::rms_zcr`. Crossings per sample, not per second.
pub fn rms_zcr(pcm: &[f32]) -> (f32, f32) {
    let n = pcm.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f32;
    let mut prev = pcm[0];
    let mut zc = 0.0f32;
    let mut i = 0usize;
    while i < n {
        let x = pcm[i];
        if !x.is_finite() {
            return (0.0, 0.0);
        }
        sum += x * x;
        if i > 0 && (prev >= 0.0) != (x >= 0.0) {
            zc += 1.0;
        }
        prev = x;
        i += 1;
    }
    let nf = n as f32;
    ((sum / nf).sqrt(), zc / nf)
}

/// One synthetic sine check. `measured_hz` is 0 when the frame was unvoiced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchCheck {
    pub target_hz: f32,
    pub measured_hz: f32,
    pub harmonicity: f32,
}

impl PitchCheck {
    pub fn error_hz(self) -> f32 {
        if self.measured_hz == 0.0 {
            f32::NAN
        } else {
            self.measured_hz - self.target_hz
        }
    }
    pub fn error_pct(self) -> f32 {
        self.error_hz() / self.target_hz * 100.0
    }
}

/// Pure sines at 24 kHz, amplitude 0.2, one frame each.
pub fn sine_checks(targets: &[f32]) -> Vec<PitchCheck> {
    targets
        .iter()
        .map(|&hz| {
            let frame = crate::synth::sine_frame(hz, 0.2);
            let p = estimate(&frame);
            PitchCheck {
                target_hz: hz,
                measured_hz: if p.voiced { p.hz } else { 0.0 },
                harmonicity: p.harmonicity,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_rms_is_amplitude_over_sqrt_two() {
        let frame = crate::synth::sine_frame(180.0, 0.2);
        let (rms, zcr) = rms_zcr(&frame);
        let expect = 0.2 / std::f32::consts::SQRT_2;
        assert!((rms - expect).abs() < 0.005, "rms {rms} expect {expect}");
        let expect_z = 2.0 * 180.0 / SAMPLE_RATE_HZ as f32;
        assert!((zcr - expect_z).abs() < 0.01, "zcr {zcr} expect {expect_z}");
    }

    #[test]
    fn known_sines_land_within_two_percent() {
        let checks = sine_checks(&[100.0, 140.0, 180.0, 220.0, 300.0]);
        for c in checks {
            assert!(c.measured_hz > 0.0, "unvoiced at {} Hz", c.target_hz);
            assert!(c.error_pct().abs() < 2.0, "{c:?} err {}%", c.error_pct());
            assert!(c.harmonicity > 0.7, "{c:?}");
        }
    }

    #[test]
    fn silence_and_noise_are_unvoiced() {
        assert!(!estimate(&[0.0; FRAME_SAMPLES]).voiced);
        let mut noise = [0.0f32; FRAME_SAMPLES];
        let mut s: u32 = 0x1234_5678;
        for n in &mut noise {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *n = ((s >> 8) as f32 / u32::MAX as f32) * 0.4 - 0.2;
        }
        let p = estimate(&noise);
        assert!(!p.voiced, "{p:?}");
    }

    #[test]
    fn short_or_nonfinite_is_unvoiced() {
        assert!(!estimate(&[0.1; 10]).voiced);
        let mut frame = [0.1f32; FRAME_SAMPLES];
        frame[10] = f32::NAN;
        assert!(!estimate(&frame).voiced);
    }
}
