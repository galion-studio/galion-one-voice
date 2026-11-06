use anyhow::{bail, Context, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

pub const TARGET_SR: u32 = 24_000;

pub fn read_mono_f32(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader = WavReader::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let spec = reader.spec();
    let ch = spec.channels as usize;
    if ch == 0 {
        bail!("wav has 0 channels");
    }
    let mut mono = Vec::new();
    match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => {
            let mut frame = Vec::with_capacity(ch);
            for s in reader.samples::<i16>() {
                frame.push(s? as f32 / i16::MAX as f32);
                if frame.len() == ch {
                    mono.push(frame.iter().sum::<f32>() / ch as f32);
                    frame.clear();
                }
            }
        }
        (SampleFormat::Int, 32) => {
            let mut frame = Vec::with_capacity(ch);
            for s in reader.samples::<i32>() {
                frame.push(s? as f32 / i32::MAX as f32);
                if frame.len() == ch {
                    mono.push(frame.iter().sum::<f32>() / ch as f32);
                    frame.clear();
                }
            }
        }
        (SampleFormat::Float, 32) => {
            let mut frame = Vec::with_capacity(ch);
            for s in reader.samples::<f32>() {
                frame.push(s?);
                if frame.len() == ch {
                    mono.push(frame.iter().sum::<f32>() / ch as f32);
                    frame.clear();
                }
            }
        }
        (fmt, bits) => bail!("unsupported wav format {:?}/{} bits", fmt, bits),
    }
    Ok((mono, spec.sample_rate))
}

pub fn write_mono_s16(path: &Path, samples: &[f32], sr: u32) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec)
        .with_context(|| format!("create {}", path.display()))?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        w.write_sample(v)?;
    }
    w.finalize()?;
    Ok(())
}

/// Fast linear resampler (good enough for ref prep; cbx re-encodes anyway).
pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = vec![0.0f32; out_len];
    let last = (input.len() - 1) as f64;
    for (i, o) in out.iter_mut().enumerate() {
        let src = (i as f64) / ratio;
        let i0 = src.floor().clamp(0.0, last) as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let t = (src - i0 as f64) as f32;
        *o = input[i0] * (1.0 - t) + input[i1] * t;
    }
    out
}

pub fn read_mono_24k(path: &Path) -> Result<Vec<f32>> {
    let (s, sr) = read_mono_f32(path)?;
    Ok(if sr == TARGET_SR {
        s
    } else {
        resample_linear(&s, sr, TARGET_SR)
    })
}

pub fn peak_dbfs(samples: &[f32]) -> f32 {
    let peak = samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    20.0 * (peak + 1e-12).log10()
}

pub fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -120.0;
    }
    let m: f32 = samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32;
    20.0 * (m.sqrt() + 1e-12).log10()
}

pub fn count_clicks(samples: &[f32], thresh: f32) -> usize {
    samples
        .windows(2)
        .filter(|w| (w[1] - w[0]).abs() > thresh)
        .count()
}
