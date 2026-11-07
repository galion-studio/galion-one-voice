//! Gain-robust log-mel speaker embedding and multi-reference averaging.
//!
//! This vector is the always-available identity prior. It is not an ECAPA embedding.
//! When a real ECAPA vector is supplied, [`average_embeddings`] averages those the same way.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub const N_MELS: usize = 24;
pub const EMBED_DIM: usize = N_MELS * 2;
const N_FFT: usize = 1024;

pub fn l2(v: &[f32]) -> f64 {
    v.iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt()
}

pub fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let n = l2(v);
    if n < 1e-12 {
        return vec![0.0; v.len()];
    }
    v.iter().map(|x| (*x as f64 / n) as f32).collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.is_empty() || a.len() != b.len() {
        return None;
    }
    let na = l2(a);
    let nb = l2(b);
    if na < 1e-12 || nb < 1e-12 {
        return None;
    }
    Some(
        a.iter()
            .zip(b)
            .map(|(x, y)| *x as f64 * *y as f64)
            .sum::<f64>()
            / (na * nb),
    )
}

/// L2-normalise each vector, take the weighted mean, L2-normalise again.
/// Zero vectors are skipped. `weights == None` weights every vector equally.
pub fn average_embeddings(vecs: &[Vec<f32>], weights: Option<&[f32]>) -> Vec<f32> {
    let dim = vecs
        .iter()
        .find(|v| !v.is_empty())
        .map(|v| v.len())
        .unwrap_or(0);
    if dim == 0 {
        return Vec::new();
    }
    let mut acc = vec![0.0f64; dim];
    let mut weight_sum = 0.0f64;
    for (i, vec) in vecs.iter().enumerate() {
        if vec.len() != dim {
            continue;
        }
        let n = l2(vec);
        if n < 1e-12 {
            continue;
        }
        let weight = weights
            .and_then(|w| w.get(i))
            .copied()
            .unwrap_or(1.0)
            .max(0.0) as f64;
        if weight == 0.0 {
            continue;
        }
        for (slot, value) in acc.iter_mut().zip(vec.iter()) {
            *slot += weight * (*value as f64 / n);
        }
        weight_sum += weight;
    }
    if weight_sum == 0.0 {
        return vec![0.0; dim];
    }
    let mean: Vec<f32> = acc.into_iter().map(|v| (v / weight_sum) as f32).collect();
    l2_normalize(&mean)
}

pub fn embedding(samples: &[f32], sr: u32) -> Vec<f32> {
    if samples.len() < 64 || sr < 1000 {
        return vec![0.0; EMBED_DIM];
    }
    let frame = ((sr as f32) * 0.025).round().max(64.0) as usize;
    let hop = ((sr as f32) * 0.010).round().max(32.0) as usize;
    let filters = mel_filters(sr, N_FFT, N_MELS);
    let window: Vec<f64> = (0..frame.min(N_FFT))
        .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / frame as f64).cos())
        .collect();
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(N_FFT);
    let mut rows: Vec<[f32; N_MELS]> = Vec::new();
    let mut pos = 0usize;
    while pos < samples.len() {
        let mut buf = vec![Complex::new(0.0, 0.0); N_FFT];
        let mut any = false;
        for i in 0..frame.min(N_FFT) {
            if pos + i >= samples.len() {
                break;
            }
            any = true;
            buf[i] = Complex::new(samples[pos + i] as f64 * window[i], 0.0);
        }
        if !any {
            break;
        }
        forward.process(&mut buf);
        let mut mel = [0.0f32; N_MELS];
        for (m, filter) in filters.iter().enumerate() {
            let mut energy = 0.0f64;
            for k in 0..=N_FFT / 2 {
                let re = buf[k].re;
                let im = buf[k].im;
                energy += (re * re + im * im) * filter[k] as f64;
            }
            mel[m] = energy.max(1e-12).ln() as f32;
        }
        let mean = mel.iter().sum::<f32>() / N_MELS as f32;
        for value in &mut mel {
            *value -= mean;
        }
        rows.push(mel);
        if pos + frame >= samples.len() {
            break;
        }
        pos += hop;
        if rows.len() > 20_000 {
            break;
        }
    }
    if rows.is_empty() {
        return vec![0.0; EMBED_DIM];
    }
    let n = rows.len() as f32;
    let mut mean = [0.0f32; N_MELS];
    let mut var = [0.0f32; N_MELS];
    for row in &rows {
        for m in 0..N_MELS {
            mean[m] += row[m];
        }
    }
    for m in 0..N_MELS {
        mean[m] /= n;
    }
    for row in &rows {
        for m in 0..N_MELS {
            let d = row[m] - mean[m];
            var[m] += d * d;
        }
    }
    let mut out = Vec::with_capacity(EMBED_DIM);
    for m in 0..N_MELS {
        out.push(mean[m]);
    }
    for m in 0..N_MELS {
        out.push((var[m] / n).max(0.0).sqrt());
    }
    l2_normalize(&out)
}

fn mel_filters(sr: u32, n_fft: usize, n_mels: usize) -> Vec<Vec<f32>> {
    let n_freq = n_fft / 2 + 1;
    let hz_to_mel = |hz: f64| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f64| 700.0 * (10f64.powf(mel / 2595.0) - 1.0);
    let f_lo = 80.0f64;
    let f_hi = (sr as f64 * 0.45).max(1000.0);
    let m0 = hz_to_mel(f_lo);
    let m1 = hz_to_mel(f_hi);
    let edges: Vec<usize> = (0..n_mels + 2)
        .map(|i| {
            let mel = m0 + (m1 - m0) * (i as f64) / (n_mels + 1) as f64;
            let hz = mel_to_hz(mel);
            let bin = (hz / sr as f64 * n_fft as f64).round() as isize;
            bin.clamp(0, n_freq as isize - 1) as usize
        })
        .collect();
    let mut bank = vec![vec![0.0f32; n_freq]; n_mels];
    for m in 0..n_mels {
        let (left, mid, right) = (edges[m], edges[m + 1], edges[m + 2]);
        if mid > left {
            for k in left..=mid {
                bank[m][k] = (k - left) as f32 / (mid - left) as f32;
            }
        }
        if right > mid {
            for k in mid..=right {
                let weight = (right - k) as f32 / (right - mid) as f32;
                if weight > bank[m][k] {
                    bank[m][k] = weight;
                }
            }
        }
        let sum: f32 = bank[m].iter().sum();
        if sum > 0.0 {
            for value in &mut bank[m] {
                *value /= sum;
            }
        }
    }
    bank
}
