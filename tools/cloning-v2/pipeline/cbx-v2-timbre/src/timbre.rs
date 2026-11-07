//! Shared reference-driven tone-colour / depth matcher (LTAS EQ).
//!
//! Derives a smoothed gain curve from each voice's own reference long-term
//! average spectrum vs the clone utterance, then applies it via STFT OLA.
//! Same code path for every voice — no per-persona hacks. No pitch shift.
//!
//! `--timbre-match 0` (default) is a no-op and must stay bit-identical.

use anyhow::{Result, bail};
use std::f32::consts::PI;
use std::path::Path;

use crate::audio::{self, TARGET_SAMPLE_RATE};

/// FFT size for LTAS / EQ (power of two).
pub const N_FFT: usize = 2048;
/// Hop for analysis / synthesis.
pub const HOP: usize = 512;
/// Max |gain| per bin in dB at strength=1 (protects ECAPA).
pub const MAX_GAIN_DB: f32 = 6.0;
/// Fractional-octave smoothing width (~1/3 octave).
pub const SMOOTH_OCTAVES: f32 = 1.0 / 3.0;

#[derive(Debug, Clone)]
pub struct LtasProfile {
    /// Smoothed power spectrum (linear), length N_FFT/2+1.
    pub power: Vec<f32>,
    /// Smoothed power in dB.
    pub power_db: Vec<f32>,
    pub sample_rate: u32,
    pub n_fft: usize,
    pub alpha_ratio_db: f32,
    pub hammarberg_db: f32,
    pub f99_hz: f32,
    /// Mean power in 150–400 Hz band (linear), for depth / body.
    pub body_power: f32,
    /// Mean power in 1–3 kHz band (linear).
    pub mid_power: f32,
}

#[derive(Debug, Clone, Default)]
pub struct TimbreMetrics {
    pub ltas_l2_db: f32,
    pub alpha_ratio_db: f32,
    pub hammarberg_db: f32,
    pub f99_hz: f32,
    pub body_mid_db: f32,
    pub alpha_delta_vs_ref_db: f32,
    pub hamm_delta_vs_ref_db: f32,
    pub f99_delta_vs_ref_hz: f32,
    pub body_mid_delta_vs_ref_db: f32,
}

/// Precompute reference LTAS (call once per voice).
pub fn profile_from_wav(path: &Path) -> Result<LtasProfile> {
    let samples = audio::read_wav_as_f32_mono_24k(path)?;
    profile_from_samples(&samples, TARGET_SAMPLE_RATE)
}

pub fn profile_from_samples(samples: &[f32], sample_rate: u32) -> Result<LtasProfile> {
    if samples.len() < N_FFT {
        bail!("audio too short for LTAS (need ≥ {N_FFT} samples)");
    }
    let power = compute_ltas_power(samples, sample_rate);
    let power_db: Vec<f32> = power
        .iter()
        .map(|&p| 10.0 * (p.max(1e-20)).log10())
        .collect();
    let power_db = smooth_fractional_octave(&power_db, sample_rate, N_FFT, SMOOTH_OCTAVES);
    let power: Vec<f32> = power_db.iter().map(|&db| 10f32.powf(db / 10.0)).collect();

    let alpha_ratio_db = band_ratio_db(&power, sample_rate, 50.0, 1000.0, 1000.0, 5000.0);
    let hammarberg_db = band_ratio_db(&power, sample_rate, 0.0, 2000.0, 2000.0, 5000.0);
    let f99_hz = f_energy_percentile(&power, sample_rate, 0.99);
    let body_power = band_mean_power(&power, sample_rate, 150.0, 400.0);
    let mid_power = band_mean_power(&power, sample_rate, 1000.0, 3000.0);

    Ok(LtasProfile {
        power,
        power_db,
        sample_rate,
        n_fft: N_FFT,
        alpha_ratio_db,
        hammarberg_db,
        f99_hz,
        body_power,
        mid_power,
    })
}

pub fn metrics_vs_ref(samples: &[f32], sample_rate: u32, ref_p: &LtasProfile) -> Result<TimbreMetrics> {
    let p = profile_from_samples(samples, sample_rate)?;
    let n = p.power_db.len().min(ref_p.power_db.len());
    let mut acc = 0.0f32;
    for i in 0..n {
        let d = p.power_db[i] - ref_p.power_db[i];
        acc += d * d;
    }
    let ltas_l2_db = (acc / n as f32).sqrt();
    let body_mid = if p.mid_power > 1e-20 {
        10.0 * (p.body_power / p.mid_power).log10()
    } else {
        0.0
    };
    let ref_body_mid = if ref_p.mid_power > 1e-20 {
        10.0 * (ref_p.body_power / ref_p.mid_power).log10()
    } else {
        0.0
    };
    Ok(TimbreMetrics {
        ltas_l2_db,
        alpha_ratio_db: p.alpha_ratio_db,
        hammarberg_db: p.hammarberg_db,
        f99_hz: p.f99_hz,
        body_mid_db: body_mid,
        alpha_delta_vs_ref_db: p.alpha_ratio_db - ref_p.alpha_ratio_db,
        hamm_delta_vs_ref_db: p.hammarberg_db - ref_p.hammarberg_db,
        f99_delta_vs_ref_hz: p.f99_hz - ref_p.f99_hz,
        body_mid_delta_vs_ref_db: body_mid - ref_body_mid,
    })
}

/// Match clone samples toward reference LTAS. `strength` in [0,1]; 0 = identity.
///
/// Strategy (shared, reference-driven — no per-voice branches):
/// 1. Build 1/3-octave-smoothed LTAS gain = ref − clone.
/// 2. Restrict correction to ~80 Hz … clone f99 (no invented air above the clone).
/// 3. Blend in explicit body (150–400 Hz) / Hammarberg / alpha soft targets.
/// 4. Clamp ±MAX_GAIN_DB, preserve RMS. No pitch shift.
pub fn match_to_ref(samples: &[f32], ref_p: &LtasProfile, strength: f32) -> Result<Vec<f32>> {
    let strength = strength.clamp(0.0, 1.0);
    if strength < 1e-6 || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    if ref_p.n_fft != N_FFT || ref_p.sample_rate != TARGET_SAMPLE_RATE {
        bail!("timbre ref profile must be 24 kHz / N_FFT={N_FFT}");
    }

    let clone_p = profile_from_samples(samples, TARGET_SAMPLE_RATE)?;
    let n_bins = N_FFT / 2 + 1;
    let f_hi = clone_p.f99_hz.clamp(2000.0, TARGET_SAMPLE_RATE as f32 * 0.45);
    let f_lo = 80.0f32;

    // Feature deltas (ref − clone): positive => need more low/body relative to high.
    let d_alpha = (ref_p.alpha_ratio_db - clone_p.alpha_ratio_db) * strength;
    let d_hamm = (ref_p.hammarberg_db - clone_p.hammarberg_db) * strength;
    let ref_bm = if ref_p.mid_power > 1e-20 {
        10.0 * (ref_p.body_power / ref_p.mid_power).max(1e-20).log10()
    } else {
        0.0
    };
    let cl_bm = if clone_p.mid_power > 1e-20 {
        10.0 * (clone_p.body_power / clone_p.mid_power).max(1e-20).log10()
    } else {
        0.0
    };
    let d_body = (ref_bm - cl_bm) * strength;

    let mut gain_db = vec![0.0f32; n_bins];
    for i in 0..n_bins {
        let freq = bin_hz(i, TARGET_SAMPLE_RATE, N_FFT);
        if freq < f_lo || freq > f_hi {
            gain_db[i] = 0.0;
            continue;
        }
        // Band-limited LTAS residual (heavily trusted only inside clone bandwidth).
        let mut g = (ref_p.power_db[i] - clone_p.power_db[i]) * strength * 0.55;

        // Gentle shelves from feature deltas (same formulas for every voice).
        // Body shelf ~250 Hz.
        g += d_body * 0.45 * shelf_weight(freq, 250.0, 0.9);
        // Alpha: more positive => boost <1 kHz / cut >1 kHz a little.
        if freq < 1000.0 {
            g += d_alpha * 0.25 * ((1000.0 - freq) / 1000.0);
        } else if freq < 5000.0 {
            g -= d_alpha * 0.20 * ((freq - 1000.0) / 4000.0).min(1.0);
        }
        // Hammarberg: positive => more 0–2 kHz vs 2–5 kHz.
        if freq < 2000.0 {
            g += d_hamm * 0.20 * ((2000.0 - freq) / 2000.0);
        } else if freq < 5000.0 {
            g -= d_hamm * 0.25 * ((freq - 2000.0) / 3000.0).min(1.0);
        }

        // Cosine taper near band edges.
        let edge = edge_taper(freq, f_lo, f_hi);
        g *= edge;
        gain_db[i] = g.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    }

    // Smooth gain curve once more (~1/3 oct) so shelves don't ring.
    gain_db = smooth_fractional_octave(&gain_db, TARGET_SAMPLE_RATE, N_FFT, SMOOTH_OCTAVES);

    // Re-zero outside band + clamp after smooth.
    for i in 0..n_bins {
        let freq = bin_hz(i, TARGET_SAMPLE_RATE, N_FFT);
        if freq < f_lo || freq > f_hi {
            gain_db[i] = 0.0;
        } else {
            gain_db[i] = gain_db[i].clamp(-MAX_GAIN_DB, MAX_GAIN_DB) * edge_taper(freq, f_lo, f_hi);
        }
    }

    let gains: Vec<f32> = gain_db.iter().map(|&db| 10f32.powf(db / 20.0)).collect();
    let mut out = apply_eq_stft(samples, &gains)?;
    let rms_in = rms(samples);
    let rms_out = rms(&out);
    if rms_out > 1e-8 && rms_in > 1e-8 {
        let scale = rms_in / rms_out;
        for s in out.iter_mut() {
            *s *= scale;
        }
    }
    Ok(out)
}

fn shelf_weight(freq: f32, center: f32, octaves: f32) -> f32 {
    let ratio = (freq / center.max(1.0)).log2().abs();
    if ratio >= octaves {
        0.0
    } else {
        0.5 * (1.0 + (std::f32::consts::PI * ratio / octaves).cos())
    }
}

fn edge_taper(freq: f32, f_lo: f32, f_hi: f32) -> f32 {
    let lo_w = 40.0f32;
    let hi_w = (f_hi * 0.15).max(200.0);
    let mut w = 1.0f32;
    if freq < f_lo + lo_w {
        w *= ((freq - f_lo) / lo_w).clamp(0.0, 1.0);
    }
    if freq > f_hi - hi_w {
        w *= ((f_hi - freq) / hi_w).clamp(0.0, 1.0);
    }
    w
}

fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
}

fn bin_hz(bin: usize, sr: u32, n_fft: usize) -> f32 {
    bin as f32 * sr as f32 / n_fft as f32
}

fn band_mean_power(power: &[f32], sr: u32, f_lo: f32, f_hi: f32) -> f32 {
    let n_fft = (power.len() - 1) * 2;
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for (i, &p) in power.iter().enumerate() {
        let f = bin_hz(i, sr, n_fft);
        if f >= f_lo && f < f_hi {
            sum += p;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

fn band_ratio_db(power: &[f32], sr: u32, a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    let lo = band_mean_power(power, sr, a0, a1).max(1e-20);
    let hi = band_mean_power(power, sr, b0, b1).max(1e-20);
    10.0 * (lo / hi).log10()
}

fn f_energy_percentile(power: &[f32], sr: u32, pct: f32) -> f32 {
    let n_fft = (power.len() - 1) * 2;
    let total: f32 = power.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let target = total * pct;
    let mut acc = 0.0f32;
    for (i, &p) in power.iter().enumerate() {
        acc += p;
        if acc >= target {
            return bin_hz(i, sr, n_fft);
        }
    }
    bin_hz(power.len() - 1, sr, n_fft)
}

fn compute_ltas_power(samples: &[f32], _sr: u32) -> Vec<f32> {
    let n_bins = N_FFT / 2 + 1;
    let mut acc = vec![0.0f32; n_bins];
    let mut frames = 0u32;
    let window = hann(N_FFT);
    // Energy gate: skip near-silent frames.
    let mut frame_energies = Vec::new();
    let mut pos = 0usize;
    while pos + N_FFT <= samples.len() {
        let mut e = 0.0f32;
        for i in 0..N_FFT {
            let s = samples[pos + i] * window[i];
            e += s * s;
        }
        frame_energies.push((pos, e));
        pos += HOP;
    }
    if frame_energies.is_empty() {
        return vec![1e-20; n_bins];
    }
    frame_energies.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let thresh = frame_energies[frame_energies.len() * 3 / 10].1; // drop quietest 30%
    let mut time_buf = vec![0.0f32; N_FFT];
    let mut freq = vec![Complex::default(); N_FFT];
    for &(pos, e) in &frame_energies {
        if e < thresh.max(1e-8) {
            continue;
        }
        for i in 0..N_FFT {
            time_buf[i] = samples[pos + i] * window[i];
        }
        for i in 0..N_FFT {
            freq[i] = Complex {
                re: time_buf[i],
                im: 0.0,
            };
        }
        fft_inplace(&mut freq, false);
        for i in 0..n_bins {
            let c = freq[i];
            acc[i] += c.re * c.re + c.im * c.im;
        }
        frames += 1;
    }
    if frames == 0 {
        return vec![1e-20; n_bins];
    }
    for a in acc.iter_mut() {
        *a /= frames as f32;
        *a = a.max(1e-20);
    }
    acc
}

fn smooth_fractional_octave(power_db: &[f32], sr: u32, n_fft: usize, octaves: f32) -> Vec<f32> {
    let n = power_db.len();
    let mut out = vec![0.0f32; n];
    let half_bw = octaves * 0.5;
    for i in 0..n {
        let f0 = bin_hz(i, sr, n_fft).max(1.0);
        let f_lo = f0 * 2f32.powf(-half_bw);
        let f_hi = f0 * 2f32.powf(half_bw);
        let mut sum = 0.0f32;
        let mut wsum = 0.0f32;
        // Bound search to nearby bins.
        let i_lo = ((f_lo * n_fft as f32 / sr as f32).floor() as usize).saturating_sub(1);
        let i_hi = ((f_hi * n_fft as f32 / sr as f32).ceil() as usize + 1).min(n - 1);
        for j in i_lo..=i_hi {
            let fj = bin_hz(j, sr, n_fft).max(1.0);
            let oct = (fj / f0).log2().abs();
            if oct <= half_bw {
                let w = 0.5 * (1.0 + (PI * oct / half_bw.max(1e-6)).cos()); // Hann in octave space
                sum += power_db[j] * w;
                wsum += w;
            }
        }
        out[i] = if wsum > 0.0 { sum / wsum } else { power_db[i] };
    }
    out
}

fn apply_eq_stft(samples: &[f32], gains: &[f32]) -> Result<Vec<f32>> {
    let n_bins = N_FFT / 2 + 1;
    if gains.len() != n_bins {
        bail!("gain curve length mismatch");
    }
    if samples.len() < N_FFT {
        return Ok(samples.to_vec());
    }
    let window = hann(N_FFT);
    let out_len = samples.len() + N_FFT;
    let mut out = vec![0.0f32; out_len];
    let mut norm = vec![0.0f32; out_len];
    let mut time_buf = vec![0.0f32; N_FFT];
    let mut freq = vec![Complex::default(); N_FFT];

    let mut pos = 0usize;
    while pos < samples.len() {
        for i in 0..N_FFT {
            let s = if pos + i < samples.len() {
                samples[pos + i]
            } else {
                0.0
            };
            time_buf[i] = s * window[i];
        }
        for i in 0..N_FFT {
            freq[i] = Complex {
                re: time_buf[i],
                im: 0.0,
            };
        }
        fft_inplace(&mut freq, false);
        // Apply real-spectrum gains (mirror for neg freqs).
        for i in 0..n_bins {
            freq[i].re *= gains[i];
            freq[i].im *= gains[i];
        }
        for i in 1..n_bins - 1 {
            let j = N_FFT - i;
            freq[j].re *= gains[i];
            freq[j].im *= gains[i];
        }
        fft_inplace(&mut freq, true);
        for i in 0..N_FFT {
            let oi = pos + i;
            if oi < out.len() {
                out[oi] += freq[i].re * window[i];
                norm[oi] += window[i] * window[i];
            }
        }
        pos += HOP;
    }
    for i in 0..out.len() {
        if norm[i] > 1e-8 {
            out[i] /= norm[i];
        }
    }
    out.truncate(samples.len());
    Ok(out)
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())
        .collect()
}

#[derive(Clone, Copy, Debug, Default)]
struct Complex {
    re: f32,
    im: f32,
}

/// In-place radix-2 Cooley–Tukey FFT. `inverse` scales by 1/n.
fn fft_inplace(buf: &mut [Complex], inverse: bool) {
    let n = buf.len();
    assert!(n.is_power_of_two());
    // Bit-reverse permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            buf.swap(i, j);
        }
    }
    let sign = if inverse { 1.0f32 } else { -1.0f32 };
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let ang = sign * 2.0 * PI / len as f32;
        let (wlen_re, wlen_im) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let mut wr = 1.0f32;
            let mut wi = 0.0f32;
            for k in 0..half {
                let u = buf[i + k];
                let v = Complex {
                    re: buf[i + k + half].re * wr - buf[i + k + half].im * wi,
                    im: buf[i + k + half].re * wi + buf[i + k + half].im * wr,
                };
                buf[i + k].re = u.re + v.re;
                buf[i + k].im = u.im + v.im;
                buf[i + k + half].re = u.re - v.re;
                buf[i + k + half].im = u.im - v.im;
                let nwr = wr * wlen_re - wi * wlen_im;
                wi = wr * wlen_im + wi * wlen_re;
                wr = nwr;
            }
        }
        len <<= 1;
    }
    if inverse {
        let inv = 1.0 / n as f32;
        for c in buf.iter_mut() {
            c.re *= inv;
            c.im *= inv;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_zero_is_identity() {
        let sr = TARGET_SAMPLE_RATE;
        let mut samples = vec![0.0f32; sr as usize];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = 0.1 * (2.0 * PI * 220.0 * i as f32 / sr as f32).sin();
        }
        let ref_p = profile_from_samples(&samples, sr).unwrap();
        let out = match_to_ref(&samples, &ref_p, 0.0).unwrap();
        assert_eq!(out, samples);
    }
}
