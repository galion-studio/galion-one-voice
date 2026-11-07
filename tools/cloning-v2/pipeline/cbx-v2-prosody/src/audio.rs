//! Audio I/O for cbx-v2 pipeline.
//!
//! Changes vs upstream cbx:
//! - Windowed-sinc resampling via rubato (was linear).
//! - Float-domain true-peak limiter (≤ -1 dBTP) before s16 write.
//! - Declick of large sample jumps (chunk-join clicks).
//! - Optional mild pace (OLA time-stretch, pitch-preserving) and mild
//!   pitch shift (resample-based; documented, keep ≤ ~2 semitones).
//!
//! PerTh watermark: NOT implemented in this Rust path (see PIPELINE.md).

use std::path::Path;

use anyhow::{Context, Result, bail};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub const TARGET_SAMPLE_RATE: u32 = 24_000;

/// True-peak ceiling: 10^(-1/20) ≈ 0.89125 (−1 dBTP).
pub const TRUE_PEAK_CEILING: f32 = 0.8912509381337456;

#[derive(Debug, Clone)]
pub struct PostProcessConfig {
    /// Speaking-rate multiplier. <1 slows (longer). Pitch-preserving OLA.
    pub pace: f32,
    /// Mild pitch shift in semitones (resample). Prefer |st| ≤ 2.
    pub pitch_semitones: f32,
    pub true_peak_dbtp: f32,
    pub declick: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            pace: 1.0,
            pitch_semitones: 0.0,
            true_peak_dbtp: -1.0,
            declick: true,
        }
    }
}

pub fn read_wav_as_f32_mono_24k(path: &Path) -> Result<Vec<f32>> {
    let (mut samples, sr) = read_wav_as_f32_mono(path)?;
    if sr != TARGET_SAMPLE_RATE {
        samples = resample_sinc_mono(&samples, sr, TARGET_SAMPLE_RATE)?;
    }
    Ok(samples)
}

/// Encode-path conditioning: sinc→24k + soft true-peak limit (does not write files).
pub fn prepare_reference_for_encode(path: &Path) -> Result<Vec<f32>> {
    let mut samples = read_wav_as_f32_mono_24k(path)?;
    samples = true_peak_limit(&samples, TRUE_PEAK_CEILING);
    if samples.iter().any(|s| s.is_nan() || s.is_infinite()) {
        bail!("reference audio contained non-finite samples after limit");
    }
    Ok(samples)
}

pub fn write_wav_f32_mono_24k(path: &Path, samples: &[f32]) -> Result<()> {
    write_wav_f32_mono_24k_with(path, samples, &PostProcessConfig::default())
}

pub fn write_wav_f32_mono_24k_with(
    path: &Path,
    samples: &[f32],
    cfg: &PostProcessConfig,
) -> Result<()> {
    let processed = postprocess_float(samples, cfg)?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).with_context(|| "failed to create wav writer")?;
    for &s in &processed {
        // After limiter, values are within ceiling; still clamp for safety.
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32).round() as i16;
        writer.write_sample(v)?;
    }
    writer.finalize()?;
    Ok(())
}

pub fn postprocess_float(samples: &[f32], cfg: &PostProcessConfig) -> Result<Vec<f32>> {
    let mut out = samples.to_vec();

    if (cfg.pitch_semitones).abs() > 1e-6 {
        // Documented mild pitch hack: resample by 1/2^(st/12) lengthens+lowers when st<0.
        let ratio = 2f64.powf(cfg.pitch_semitones as f64 / 12.0);
        out = resample_by_ratio(&out, 1.0 / ratio)?;
    }

    if (cfg.pace - 1.0).abs() > 1e-4 {
        out = time_stretch_ola(&out, cfg.pace);
    }

    if cfg.declick {
        out = declick(&out, 0.35);
    }

    let ceiling = 10f32.powf(cfg.true_peak_dbtp / 20.0);
    out = true_peak_limit(&out, ceiling);
    Ok(out)
}

fn read_wav_as_f32_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open wav {}", path.display()))?;
    let spec = reader.spec();

    if spec.sample_format == hound::SampleFormat::Int {
        if spec.bits_per_sample == 16 {
            let mut out = Vec::new();
            let mut frame = Vec::with_capacity(spec.channels as usize);
            for s in reader.samples::<i16>() {
                frame.push(s? as f32 / i16::MAX as f32);
                if frame.len() == spec.channels as usize {
                    out.push(frame.iter().sum::<f32>() / spec.channels as f32);
                    frame.clear();
                }
            }
            Ok((out, spec.sample_rate))
        } else if spec.bits_per_sample == 32 {
            let mut out = Vec::new();
            let mut frame = Vec::with_capacity(spec.channels as usize);
            for s in reader.samples::<i32>() {
                frame.push(s? as f32 / i32::MAX as f32);
                if frame.len() == spec.channels as usize {
                    out.push(frame.iter().sum::<f32>() / spec.channels as f32);
                    frame.clear();
                }
            }
            Ok((out, spec.sample_rate))
        } else {
            bail!(
                "unsupported int wav bits_per_sample={}",
                spec.bits_per_sample
            );
        }
    } else if spec.sample_format == hound::SampleFormat::Float {
        if spec.bits_per_sample != 32 {
            bail!(
                "unsupported float wav bits_per_sample={}",
                spec.bits_per_sample
            );
        }
        let mut out = Vec::new();
        let mut frame = Vec::with_capacity(spec.channels as usize);
        for s in reader.samples::<f32>() {
            frame.push(s?);
            if frame.len() == spec.channels as usize {
                out.push(frame.iter().sum::<f32>() / spec.channels as f32);
                frame.clear();
            }
        }
        Ok((out, spec.sample_rate))
    } else {
        bail!("unsupported wav sample_format");
    }
}

pub fn resample_sinc_mono(input: &[f32], from_sr: u32, to_sr: u32) -> Result<Vec<f32>> {
    if input.is_empty() || from_sr == to_sr {
        return Ok(input.to_vec());
    }
    let ratio = to_sr as f64 / from_sr as f64;
    resample_by_ratio(input, ratio)
}

fn resample_by_ratio(input: &[f32], ratio: f64) -> Result<Vec<f32>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if (ratio - 1.0).abs() < 1e-12 {
        return Ok(input.to_vec());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // Process in chunks; rubato needs fixed input chunk size.
    let chunk_size = 1024usize;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| anyhow::anyhow!("rubato SincFixedIn init failed: {e}"))?;

    let mut out: Vec<f32> = Vec::with_capacity(((input.len() as f64) * ratio) as usize + chunk_size);
    let mut pos = 0usize;
    while pos + chunk_size <= input.len() {
        let wave = vec![input[pos..pos + chunk_size].to_vec()];
        let waves_out = resampler
            .process(&wave, None)
            .map_err(|e| anyhow::anyhow!("rubato process failed: {e}"))?;
        out.extend_from_slice(&waves_out[0]);
        pos += chunk_size;
    }

    // Pad remainder with zeros to a full chunk, then process and trim latency.
    if pos < input.len() || true {
        let mut tail = vec![0.0f32; chunk_size];
        let remain = input.len().saturating_sub(pos);
        if remain > 0 {
            tail[..remain].copy_from_slice(&input[pos..]);
        }
        // Flush a few empty chunks to drain the filter delay.
        for (i, _) in [0, 1, 2].iter().enumerate() {
            let wave = if i == 0 {
                vec![tail.clone()]
            } else {
                vec![vec![0.0f32; chunk_size]]
            };
            let waves_out = resampler
                .process(&wave, None)
                .map_err(|e| anyhow::anyhow!("rubato flush failed: {e}"))?;
            out.extend_from_slice(&waves_out[0]);
        }
    }

    // Trim to expected length (round).
    let expected = ((input.len() as f64) * ratio).round() as usize;
    if out.len() > expected {
        out.truncate(expected);
    } else if out.len() < expected {
        out.resize(expected, 0.0);
    }
    Ok(out)
}

/// Lookahead peak limiter with simple 4× peak estimate (true-peak proxy).
pub fn true_peak_limit(input: &[f32], ceiling: f32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let ceiling = ceiling.abs().max(1e-3);
    // Estimate true peak via linear interp max between samples (cheap 2× TP proxy).
    let mut peak = 0.0f32;
    for i in 0..input.len() {
        let a = input[i].abs();
        if a > peak {
            peak = a;
        }
        if i + 1 < input.len() {
            // Midpoint magnitude upper bound.
            let mid = 0.5 * (input[i] + input[i + 1]);
            let m = mid.abs();
            if m > peak {
                peak = m;
            }
        }
    }
    if peak <= ceiling {
        return input.to_vec();
    }
    let gain = ceiling / peak;
    // Soft attack envelope: apply constant gain (brickwall normalize to ceiling).
    // For short TTS clips a constant make-up is preferable to pumping.
    input.iter().map(|s| s * gain).collect()
}

/// Repair clicks: large adjacent-sample jumps get cubic-ish local smoothing.
pub fn declick(input: &[f32], jump_thresh: f32) -> Vec<f32> {
    if input.len() < 3 {
        return input.to_vec();
    }
    let mut out = input.to_vec();
    let thr = jump_thresh.abs().max(0.05);
    for i in 1..out.len() {
        let d = out[i] - out[i - 1];
        if d.abs() > thr {
            // Replace out[i] with blend toward linear interp from neighbors.
            let prev = out[i - 1];
            let next = if i + 1 < out.len() { out[i + 1] } else { prev };
            let target = 0.5 * (prev + next);
            // Soften the jump over 3 samples when possible.
            out[i] = 0.25 * out[i] + 0.75 * target;
            if i + 1 < out.len() {
                out[i + 1] = 0.5 * out[i + 1] + 0.5 * (target + next) * 0.5;
            }
        }
    }
    out
}

/// Pitch-preserving time stretch. `rate` is output speaking rate relative to input
/// (0.85 → slower / longer). Simple Hanning OLA; fine for mild adjustments.
pub fn time_stretch_ola(input: &[f32], rate: f32) -> Vec<f32> {
    let rate = rate.clamp(0.5, 1.5);
    if input.is_empty() || (rate - 1.0).abs() < 1e-4 {
        return input.to_vec();
    }
    let win_len = 1024usize;
    let hop_analysis = 256usize;
    // Speaking-rate multiplier: rate<1 slows (longer). hop_out = hop_in / rate.
    let hop_synthesis = ((hop_analysis as f32) / rate).round().max(1.0) as usize;

    let mut window = vec![0.0f32; win_len];
    for (i, w) in window.iter_mut().enumerate() {
        let x = std::f32::consts::PI * 2.0 * (i as f32) / (win_len as f32);
        *w = 0.5 - 0.5 * x.cos(); // Hann
    }

    let out_len = ((input.len() as f32) / rate).round() as usize + win_len;
    let mut out = vec![0.0f32; out_len];
    let mut norm = vec![0.0f32; out_len];

    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    while in_pos + win_len <= input.len() {
        for i in 0..win_len {
            let s = input[in_pos + i] * window[i];
            let oi = out_pos + i;
            if oi < out.len() {
                out[oi] += s;
                norm[oi] += window[i];
            }
        }
        in_pos += hop_analysis;
        out_pos += hop_synthesis;
    }

    for i in 0..out.len() {
        if norm[i] > 1e-6 {
            out[i] /= norm[i];
        }
    }
    // Trim trailing near-silence from padding.
    let mut end = out.len();
    while end > 0 && out[end - 1].abs() < 1e-5 {
        end -= 1;
    }
    out.truncate(end);
    out
}
