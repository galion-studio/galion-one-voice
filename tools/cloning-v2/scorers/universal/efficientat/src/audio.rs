//! WAV load + linear resample to 32 kHz for EfficientAT.
use anyhow::{Context, Result};
use std::path::Path;

pub fn resample_linear(x: &[f32], sr_in: u32, sr_out: u32) -> Vec<f32> {
    if sr_in == sr_out || x.is_empty() {
        return x.to_vec();
    }
    let n_out = ((x.len() as u64 * sr_out as u64 + (sr_in as u64 / 2)) / sr_in as u64) as usize;
    let n_out = n_out.max(1);
    let mut y = vec![0.0f32; n_out];
    let scale = sr_in as f64 / sr_out as f64;
    for (i, yi) in y.iter_mut().enumerate() {
        let src = i as f64 * scale;
        let j = src.floor() as usize;
        let f = (src - j as f64) as f32;
        let a = x[j.min(x.len() - 1)];
        let b = x[(j + 1).min(x.len() - 1)];
        *yi = a + (b - a) * f;
    }
    y
}

pub fn load_mono_resampled(path: &Path, target_sr: u32) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample.min(31) - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    let mono: Vec<f32> = raw.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    let y = resample_linear(&mono, spec.sample_rate, target_sr);
    Ok((y, target_sr))
}
