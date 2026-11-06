//! Read-only WAV decode. Callers write nothing back to the source path.

use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn read_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        bail!("invalid WAV format: {}", path.display());
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.map_err(Into::into))
            .collect::<Result<_>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale).map_err(Into::into))
                .collect::<Result<_>>()?
        }
    };
    if samples.iter().any(|s| !s.is_finite()) {
        bail!("non-finite samples in {}", path.display());
    }
    let ch = spec.channels as usize;
    let mono = samples
        .chunks_exact(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

/// Linear resample. Same interpolator as the persona scorer's `load_wav`.
pub fn resample(x: &[f32], from_sr: u32, to_sr: u32) -> Vec<f32> {
    if from_sr == to_sr || x.is_empty() {
        return x.to_vec();
    }
    let n = ((x.len() as f64) * (to_sr as f64) / (from_sr as f64)).round() as usize;
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let last = (x.len() - 1) as f64;
    for i in 0..n {
        let pos = (i as f64) * (from_sr as f64) / (to_sr as f64);
        let pos = pos.min(last);
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(x.len() - 1);
        let t = (pos - lo as f64) as f32;
        out.push(x[lo] * (1.0 - t) + x[hi] * t);
    }
    out
}

pub fn read_mono_16k(path: &Path) -> Result<Vec<f32>> {
    let (mono, sr) = read_mono(path)?;
    Ok(resample(&mono, sr, crate::prosody::SR as u32))
}
