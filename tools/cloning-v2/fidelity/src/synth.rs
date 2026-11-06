//! Deterministic test signals. Not a voice, not stored.

use crate::audio::TARGET_SR;

pub fn lcg_noise(n: usize, amp: f32, seed: u32) -> Vec<f32> {
    let mut state = seed | 1;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let unit = ((state >> 8) as f32) / 16_777_216.0;
            (unit * 2.0 - 1.0) * amp
        })
        .collect()
}

pub fn harmonics(sr: u32, dur_s: f32, f0: f32, amp: f32) -> Vec<f32> {
    let n = ((sr as f32) * dur_s).round().max(1.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let mut s = 0.0f32;
            for h in 1..=4 {
                s += (1.0 / h as f32) * (2.0 * std::f32::consts::PI * f0 * h as f32 * t).sin();
            }
            s * amp
        })
        .collect()
}

pub fn glide(sr: u32, dur_s: f32, f0_a: f32, f0_b: f32, amp: f32) -> Vec<f32> {
    let n = ((sr as f32) * dur_s).round().max(1.0) as usize;
    let mut phase = [0.0f32; 4];
    let mut out = vec![0.0f32; n];
    for i in 0..n {
        let t = if n <= 1 {
            0.0
        } else {
            i as f32 / (n - 1) as f32
        };
        let f0 = f0_a + (f0_b - f0_a) * t;
        let mut s = 0.0f32;
        for h in 1..=4 {
            phase[h - 1] += 2.0 * std::f32::consts::PI * f0 * h as f32 / sr as f32;
            s += (1.0 / h as f32) * phase[h - 1].sin();
        }
        out[i] = s * amp;
    }
    out
}

pub fn concat(parts: &[Vec<f32>]) -> Vec<f32> {
    let mut out = Vec::new();
    for part in parts {
        out.extend_from_slice(part);
    }
    out
}

pub fn seconds(n: usize) -> f32 {
    n as f32 / TARGET_SR as f32
}

pub fn emphasize_high(samples: &[f32], sr: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let dt = 1.0 / sr as f32;
    let rc = 1.0 / (2.0 * std::f32::consts::PI * 3000.0);
    let a = rc / (rc + dt);
    let mut hp = vec![0.0f32; samples.len()];
    let mut prev_y = 0.0f32;
    let mut prev_x = samples[0];
    for i in 1..samples.len() {
        prev_y = a * (prev_y + samples[i] - prev_x);
        prev_x = samples[i];
        hp[i] = prev_y;
    }
    samples.iter().zip(hp).map(|(s, h)| s + 4.0 * h).collect()
}
