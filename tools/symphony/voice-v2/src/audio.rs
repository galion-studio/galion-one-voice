// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! WAV in and out, a synthetic vowel, and the small measures the glue needs.
//! The vowel is a test tone: a pulse train through two resonators. It is not
//! a voice and it is not a model of one.

use std::path::Path;

/// Steady vowel. Same shape as the articulation crate's synthetic fixture.
pub fn vowel(f0: f64, f1: f64, f2: f64, secs: f64, sr: f64) -> Vec<f64> {
    let n = (secs * sr).round().max(1.0) as usize;
    let mut src = vec![0.0; n];
    let period = (sr / f0.max(40.0)).round().max(1.0) as usize;
    let mut i = 0;
    while i < n {
        src[i] = 1.0;
        i += period;
    }
    let mut y = src;
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

pub fn write_wav(path: &Path, samples: &[f64], sr: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w =
        hound::WavWriter::create(path, spec).map_err(|e| format!("{}: {e}", path.display()))?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

pub fn read_wav(path: &Path) -> Result<(Vec<f64>, f64), String> {
    galion_articulation::acoustic::read_wav_mono(&path.to_string_lossy())
}

pub fn rms_ticks(samples: &[f64], sr: f64, n: usize) -> Vec<Option<f64>> {
    let hop = (sr * crate::TICK_S).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = i * hop;
        if a >= samples.len() {
            out.push(None);
            continue;
        }
        let b = ((i + 1) * hop).min(samples.len());
        let slice = &samples[a..b];
        if slice.is_empty() {
            out.push(None);
            continue;
        }
        let mean_sq = slice.iter().map(|s| s * s).sum::<f64>() / slice.len() as f64;
        out.push(Some(mean_sq.sqrt()));
    }
    out
}

pub fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

pub fn apply_gain(samples: &mut [f64], gain: f64) {
    if !gain.is_finite() {
        return;
    }
    for s in samples {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}
