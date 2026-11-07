// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Synthetic sine and noise for the checker tests.
//!
//! These are not a voice, not a recording of a person, and not a clone.
//! Nothing here is written into the repository.

use crate::dsp::{fft, ifft, integrated_lufs};

const PI: f64 = std::f64::consts::PI;

fn xorshift(state: &mut u64) -> u64 {
    if *state == 0 {
        *state = 0x9E37_79B9_7F4A_7C15;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn gauss(state: &mut u64) -> f64 {
    let u1 = ((xorshift(state) as f64) + 1.0) / (u64::MAX as f64 + 2.0);
    let u2 = (xorshift(state) as f64) / (u64::MAX as f64 + 1.0);
    let u1 = u1.clamp(1e-12, 1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos())
        .collect()
}

/// Noise whose power spectral density falls by `tilt_db_per_octave`.
pub fn shaped_noise(n: usize, sr: u32, tilt_db_per_octave: f64, seed: u64) -> Vec<f32> {
    let nfft = 8192usize;
    let hop = nfft / 2;
    let extra = nfft;
    let total = n + extra;
    let mut out = vec![0.0; total + nfft];
    let mut norm = vec![0.0; total + nfft];
    let win = hann(nfft);
    let mut rng = seed | 1;
    let beta = tilt_db_per_octave / (20.0 * 2.0_f64.log10());
    let mut pos = 0usize;
    while pos < total {
        let mut re = vec![0.0; nfft];
        let mut im = vec![0.0; nfft];
        for i in 0..nfft {
            re[i] = gauss(&mut rng) * win[i];
        }
        fft(&mut re, &mut im);
        for k in 0..=nfft / 2 {
            let f = k as f64 * sr as f64 / nfft as f64;
            let g = if f >= 30.0 {
                (f / 432.0).powf(beta)
            } else {
                0.0
            };
            re[k] *= g;
            im[k] *= g;
        }
        for k in 1..nfft / 2 {
            re[nfft - k] = re[k];
            im[nfft - k] = -im[k];
        }
        im[0] = 0.0;
        im[nfft / 2] = 0.0;
        ifft(&mut re, &mut im);
        for i in 0..nfft {
            out[pos + i] += re[i] * win[i];
            norm[pos + i] += win[i] * win[i];
        }
        pos += hop;
    }
    let mut y = vec![0.0f32; n];
    let crop = extra / 2;
    for i in 0..n {
        let d = norm[crop + i].max(1e-8).sqrt();
        y[i] = (out[crop + i] / d) as f32;
    }
    y
}

pub fn sine(n: usize, sr: u32, hz: f64, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (2.0 * PI * hz * i as f64 / sr as f64).sin() as f32)
        .collect()
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len() as f64).sqrt()
}

fn scale_rms(x: &mut [f32], target: f64) {
    let r = rms(x);
    if r > 0.0 {
        let g = (target / r) as f32;
        for s in x {
            *s *= g;
        }
    }
}

pub fn remove_dc(x: &mut [f32]) {
    if x.is_empty() {
        return;
    }
    let m = x.iter().map(|s| *s as f64).sum::<f64>() / x.len() as f64;
    for s in x {
        *s -= m as f32;
    }
}

pub fn scale_to_lufs(x: &mut [f32], sr: u32, target: f64) -> Result<(), String> {
    let m =
        integrated_lufs(x, sr, 400, 100, -70.0, 10.0).ok_or("synth: loudness is not measurable")?;
    let g = 10f64.powf((target - m) / 20.0) as f32;
    for s in x {
        *s *= g;
    }
    Ok(())
}

fn bed(n: usize, dbfs: f64, seed: u64) -> Vec<f32> {
    let amp = 10f64.powf(dbfs / 20.0) as f32;
    let mut rng = seed | 1;
    (0..n).map(|_| amp * gauss(&mut rng) as f32).collect()
}

fn embed(body: &[f32], sr: u32, target_lufs: f64, seed: u64) -> Result<Vec<f32>, String> {
    let lead = (sr as f64 * 0.50) as usize;
    let trail = (sr as f64 * 0.55) as usize;
    let mut x = bed(lead + body.len() + trail, -78.0, seed);
    for (i, s) in body.iter().enumerate() {
        x[lead + i] += *s;
    }
    remove_dc(&mut x);
    scale_to_lufs(&mut x, sr, target_lufs)?;
    remove_dc(&mut x);
    Ok(x)
}

/// Shaped-noise burst with a quiet bed and an exponential tail.
/// `tau_s == 0` is a hard stop (dry). Level is integrated LUFS after the bed is mixed.
pub fn burst(
    sr: u32,
    tilt_db_per_octave: f64,
    tau_s: f64,
    target_lufs: f64,
    seed: u64,
) -> Result<Vec<f32>, String> {
    let sustain_n = (sr as f64 * 2.4) as usize;
    let mut sustain = shaped_noise(sustain_n, sr, tilt_db_per_octave, seed);
    let fade = (sr as f64 * 0.02) as usize;
    for i in 0..fade.min(sustain.len()) {
        sustain[i] *= i as f32 / fade as f32;
    }
    scale_rms(&mut sustain, 0.1);
    let mut body = sustain;
    if tau_s > 0.0 {
        let tail_n = ((sr as f64 * 0.8).max(sr as f64 * 6.0 * tau_s) as usize)
            .min((sr as f64 * 4.0) as usize);
        let mut tail = shaped_noise(tail_n, sr, tilt_db_per_octave, seed.wrapping_add(17));
        scale_rms(&mut tail, 0.1);
        let dt = 1.0 / sr as f64;
        for (i, s) in tail.iter_mut().enumerate() {
            *s *= (-(i as f64) * dt / tau_s).exp() as f32;
        }
        body.extend_from_slice(&tail);
    }
    remove_dc(&mut body);
    scale_to_lufs(&mut body, sr, target_lufs)?;
    embed(&body, sr, target_lufs, seed.wrapping_add(99))
}

/// 432 Hz calibration tone at `target_lufs`, with a quiet bed and short edge fades.
pub fn calibration_tone(sr: u32, target_lufs: f64) -> Result<Vec<f32>, String> {
    let n = (sr as f64 * 3.0) as usize;
    let mut tone = sine(n, sr, 432.0, 1.0);
    let fade = (sr as f64 * 0.005) as usize;
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        tone[i] *= g;
        tone[n - 1 - i] *= g;
    }
    remove_dc(&mut tone);
    scale_to_lufs(&mut tone, sr, target_lufs)?;
    embed(&tone, sr, target_lufs, 0x432)
}
