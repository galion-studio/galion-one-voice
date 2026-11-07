//! Opt-in spectral-envelope correction.
//!
//! Cepstral lifter (1.5 ms) of the log power spectrum, then a ±6 dB 1/3-octave-smoothed
//! shape match. Strength 0 is bit-identical. Loudness is restored; pitch is not shifted.
//! The layer is off unless a caller asks for it.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use crate::analyze;
use crate::audio;

const N_FFT: usize = 1024;
const MAX_GAIN_DB: f64 = 6.0;

pub fn correct_envelope(input: &[f32], reference: &[f32], sr: u32, strength: f32) -> Vec<f32> {
    if !(strength > 1e-6) || input.is_empty() {
        return input.to_vec();
    }
    let strength = (strength as f64).clamp(0.0, 1.0);
    if input.len() < N_FFT || reference.len() < N_FFT / 2 || sr < 1000 {
        return input.to_vec();
    }
    let env_in = log_envelope(&analyze::welch_power(input, N_FFT), sr);
    let env_ref = log_envelope(&analyze::welch_power(reference, N_FFT), sr);
    let n = env_in.len().min(env_ref.len());
    let mean = |env: &[f64]| {
        if n <= 2 {
            return 0.0;
        }
        env[1..n - 1].iter().sum::<f64>() / (n - 2) as f64
    };
    let mean_ref = mean(&env_ref);
    let mean_in = mean(&env_in);
    let mut gain_db = vec![0.0f64; n];
    for k in 0..n {
        let db = ((env_ref[k] - mean_ref) - (env_in[k] - mean_in))
            * (10.0 / std::f64::consts::LN_10)
            * strength;
        gain_db[k] = db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    }
    gain_db = smooth_third_octave(&gain_db, sr, N_FFT);
    let mut gain_lin = vec![1.0f64; n];
    let nyquist_hi = sr as f64 * 0.45;
    let taper_hi = sr as f64 * 0.40;
    for k in 0..n {
        let freq = k as f64 * sr as f64 / N_FFT as f64;
        let taper = if freq < 80.0 {
            0.0
        } else if freq < 150.0 {
            0.5 - 0.5 * ((freq - 80.0) / 70.0 * std::f64::consts::PI).cos()
        } else if freq > nyquist_hi {
            0.0
        } else if freq > taper_hi {
            let t = ((freq - taper_hi) / (nyquist_hi - taper_hi)).clamp(0.0, 1.0);
            0.5 + 0.5 * (t * std::f64::consts::PI).cos()
        } else {
            1.0
        };
        let db = (gain_db[k] * taper).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        gain_lin[k] = 10f64.powf(db / 20.0);
    }
    let mut output = apply_gain(input, &gain_lin);
    let in_rms = audio::rms(input);
    let out_rms = audio::rms(&output);
    if in_rms > 1e-8 && out_rms > 1e-8 {
        let g = in_rms / out_rms;
        for sample in &mut output {
            *sample *= g;
        }
    }
    let peak = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 0.99 {
        let g = 0.99 / peak;
        for sample in &mut output {
            *sample *= g;
        }
    }
    if output.iter().any(|s| !s.is_finite()) {
        return input.to_vec();
    }
    output
}

pub fn apply_gain(input: &[f32], gain_lin: &[f64]) -> Vec<f32> {
    if gain_lin.len() < 2 || input.is_empty() {
        return input.to_vec();
    }
    let n_fft = (gain_lin.len() - 1) * 2;
    let hop = n_fft / 2;
    let pad = hop;
    let mut padded = vec![0.0f32; input.len() + 2 * pad];
    padded[pad..pad + input.len()].copy_from_slice(input);
    let window: Vec<f64> = (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n_fft as f64).cos())
        .collect();
    let mut acc = vec![0.0f64; padded.len() + n_fft];
    let mut norm = vec![0.0f64; padded.len() + n_fft];
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n_fft);
    let inverse = planner.plan_fft_inverse(n_fft);
    let mut pos = 0usize;
    while pos < padded.len() {
        let mut buf = vec![Complex::new(0.0, 0.0); n_fft];
        for i in 0..n_fft {
            let sample = if pos + i < padded.len() {
                padded[pos + i] as f64
            } else {
                0.0
            };
            buf[i] = Complex::new(sample * window[i], 0.0);
        }
        forward.process(&mut buf);
        for k in 0..=n_fft / 2 {
            let g = gain_lin.get(k).copied().unwrap_or(1.0);
            buf[k] = Complex::new(buf[k].re * g, buf[k].im * g);
            if k > 0 && k < n_fft / 2 {
                let mirror = n_fft - k;
                buf[mirror] = Complex::new(buf[mirror].re * g, buf[mirror].im * g);
            }
        }
        inverse.process(&mut buf);
        let scale = 1.0 / n_fft as f64;
        for i in 0..n_fft {
            let idx = pos + i;
            if idx >= acc.len() {
                break;
            }
            acc[idx] += buf[i].re * scale * window[i];
            norm[idx] += window[i] * window[i];
        }
        pos += hop;
    }
    (0..input.len())
        .map(|i| {
            let idx = i + pad;
            if norm[idx] > 1e-8 {
                (acc[idx] / norm[idx]) as f32
            } else {
                0.0
            }
        })
        .collect()
}

fn log_envelope(power: &[f64], sr: u32) -> Vec<f64> {
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(N_FFT);
    let inverse = planner.plan_fft_inverse(N_FFT);
    let mut spec = vec![Complex::new(0.0, 0.0); N_FFT];
    for k in 0..=N_FFT / 2 {
        let p = power.get(k).copied().unwrap_or(1e-20).max(1e-20);
        let log_p = p.ln();
        spec[k] = Complex::new(log_p, 0.0);
        if k > 0 && k < N_FFT / 2 {
            spec[N_FFT - k] = Complex::new(log_p, 0.0);
        }
    }
    inverse.process(&mut spec);
    let scale = 1.0 / N_FFT as f64;
    for bin in &mut spec {
        *bin = Complex::new(bin.re * scale, 0.0);
    }
    let lifter = ((0.0015 * sr as f64).round() as usize).clamp(8, N_FFT / 2);
    for n in 0..N_FFT {
        let q = if n <= N_FFT / 2 { n } else { N_FFT - n };
        if q > lifter {
            spec[n] = Complex::new(0.0, 0.0);
        } else if q == lifter {
            spec[n].re *= 0.5;
        }
    }
    forward.process(&mut spec);
    (0..=N_FFT / 2).map(|k| spec[k].re).collect()
}

fn smooth_third_octave(db: &[f64], sr: u32, n_fft: usize) -> Vec<f64> {
    let factor = 2f64.powf(1.0 / 6.0);
    let mut out = vec![0.0; db.len()];
    for i in 0..db.len() {
        let freq = i as f64 * sr as f64 / n_fft as f64;
        if freq < 1.0 {
            out[i] = db[i];
            continue;
        }
        let lo = freq / factor;
        let hi = freq * factor;
        let mut sum = 0.0;
        let mut count = 0.0;
        for (j, value) in db.iter().enumerate() {
            let fj = j as f64 * sr as f64 / n_fft as f64;
            if fj >= lo && fj <= hi {
                sum += value;
                count += 1.0;
            }
        }
        out[i] = if count > 0.0 { sum / count } else { db[i] };
    }
    out
}

pub fn high_low_ratio(samples: &[f32], sr: u32) -> f64 {
    let power = analyze::welch_power(samples, N_FFT);
    let mut high = 0.0;
    let mut low = 0.0;
    for (k, value) in power.iter().enumerate() {
        let freq = k as f64 * sr as f64 / N_FFT as f64;
        if (3000.0..8000.0).contains(&freq) {
            high += value;
        }
        if (80.0..1000.0).contains(&freq) {
            low += value;
        }
    }
    high / low.max(1e-12)
}
