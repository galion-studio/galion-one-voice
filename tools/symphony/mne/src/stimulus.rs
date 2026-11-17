// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! A pulse-train vowel used as a CI stimulus. Not a voice, not a cloner,
//! and not the sound-out layer (S6).

pub const SR: f64 = 16_000.0;

pub fn vowel(f0: f64, f1: f64, f2: f64, secs: f64, sr: f64) -> Vec<f64> {
    let n = (secs * sr).round().max(1.0) as usize;
    let mut y = vec![0.0; n];
    let period = (sr / f0.max(40.0)).round().max(1.0) as usize;
    let mut i = 0;
    while i < n {
        y[i] = 1.0;
        i += period;
    }
    for (f, bw) in [(f1, 80.0), (f2, 120.0)] {
        let r = (-std::f64::consts::PI * bw / sr).exp();
        let c = 2.0 * r * (2.0 * std::f64::consts::PI * f / sr).cos();
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in y.iter_mut() {
            let o = *v + c * y1 - r * r * y2;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let m = y.iter().fold(0.0_f64, |a, b| a.max(b.abs())).max(1e-9);
    y.iter().map(|v| 0.5 * v / m).collect()
}
