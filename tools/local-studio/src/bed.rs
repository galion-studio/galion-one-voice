// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Music and effects bed, generated in this file.
//!
//! The tuning is 432 Hz. Pad, chime, and low pulse are integer ratios of
//! that fundamental. Breath is filtered noise from a fixed generator.
//! There is no sample file and no donor clip.

use crate::audio::fade_edges;

const PI: f64 = std::f64::consts::PI;

pub fn render_bed(n: usize, sample_rate: u32, tuning_hz: f64) -> Vec<f32> {
    let mut samples = vec![0.0f32; n];
    if n == 0 || sample_rate == 0 || !tuning_hz.is_finite() || tuning_hz <= 0.0 {
        return samples;
    }
    add_pad(&mut samples, sample_rate, tuning_hz);
    add_chime(&mut samples, sample_rate, tuning_hz);
    add_pulse(&mut samples, sample_rate, tuning_hz);
    add_breath(&mut samples, sample_rate);
    fade_edges(&mut samples, sample_rate, 30);
    samples
}

fn add_pad(samples: &mut [f32], sample_rate: u32, tuning_hz: f64) {
    let sr = f64::from(sample_rate);
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let trem = 0.85 + 0.15 * (2.0 * PI * 0.08 * t).sin();
        let pad = (2.0 * PI * tuning_hz * t).sin()
            + 0.35 * (2.0 * PI * tuning_hz * 2.0 * t).sin()
            + 0.18 * (2.0 * PI * tuning_hz * 1.5 * t).sin();
        *sample += (0.22 * trem * pad) as f32;
    }
}

fn add_chime(samples: &mut [f32], sample_rate: u32, tuning_hz: f64) {
    let sr = f64::from(sample_rate);
    let period = (sr * 5.0) as usize;
    if period == 0 {
        return;
    }
    let partials = [1.0, 2.0, 3.0, 4.0];
    let amps = [0.50, 0.25, 0.12, 0.06];
    let decay_n = (sr * 1.2) as usize;
    let mut strike = 0usize;
    while strike < samples.len() {
        for k in 0..decay_n {
            let i = strike + k;
            if i >= samples.len() {
                break;
            }
            let t = k as f64 / sr;
            let env = (-t / 0.35).exp();
            let mut acc = 0.0;
            for (partial, amp) in partials.iter().zip(amps) {
                let phase = 2.0 * PI * tuning_hz * partial * (i as f64) / sr;
                acc += amp * phase.sin();
            }
            samples[i] += (0.18 * env * acc) as f32;
        }
        strike += period;
    }
}

fn add_pulse(samples: &mut [f32], sample_rate: u32, tuning_hz: f64) {
    let sr = f64::from(sample_rate);
    let pulse_hz = tuning_hz / 4.0;
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let env = (0.5 + 0.5 * (2.0 * PI * 0.5 * t).sin()).powi(2);
        let tone = (2.0 * PI * pulse_hz * t).sin();
        *sample += (0.16 * env * tone) as f32;
    }
}

fn add_breath(samples: &mut [f32], sample_rate: u32) {
    let sr = f64::from(sample_rate);
    let mut state = 0x5A17_C0DEu32;
    let mut hp = 0.0f64;
    let mut prev = 0.0f64;
    let dt = 1.0 / sr;
    let rc = 1.0 / (2.0 * PI * 700.0);
    let a = rc / (rc + dt);
    for (i, sample) in samples.iter_mut().enumerate() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let white = ((state >> 8) as f64 / 16_777_216.0) * 2.0 - 1.0;
        hp = a * (hp + white - prev);
        prev = white;
        let t = i as f64 / sr;
        let slow = 0.6 + 0.4 * (2.0 * PI * 0.13 * t).sin();
        *sample += (0.03 * slow * hp) as f32;
    }
}

/// Single-bin power, used to check that the bed is tuned to 432 Hz.
pub fn tone_power(samples: &[f32], sample_rate: u32, freq_hz: f64) -> f64 {
    let n = samples.len();
    if n == 0 || sample_rate == 0 {
        return 0.0;
    }
    let k = (0.5 + (n as f64 * freq_hz / f64::from(sample_rate))).floor();
    let w = 2.0 * PI * k / n as f64;
    let coeff = 2.0 * w.cos();
    let mut s1 = 0.0;
    let mut s2 = 0.0;
    for sample in samples {
        let s0 = f64::from(*sample) + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2) / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bed_is_tuned_to_432_and_is_deterministic() {
        let sr = 24_000u32;
        let a = render_bed(sr as usize * 3, sr, 432.0);
        let b = render_bed(sr as usize * 3, sr, 432.0);
        assert_eq!(a, b);
        let p432 = tone_power(&a, sr, 432.0);
        let p440 = tone_power(&a, sr, 440.0);
        assert!(p432 > p440 * 8.0, "432 {p432} 440 {p440}");
        let p108 = tone_power(&a, sr, 108.0);
        assert!(p108 > p440, "108 {p108} 440 {p440}");
    }
}
