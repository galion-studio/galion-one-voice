//! WAV IO and small signal helpers. This crate never writes inside the git tree.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const TARGET_SR: u32 = 24_000;

pub fn sanitize(y: &[f32]) -> Vec<f32> {
    y.iter()
        .map(|&v| if v.is_finite() { v } else { 0.0 })
        .collect()
}

pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

pub fn percentile(xs: &[f32], q: f32) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = xs.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (((s.len() - 1) as f32) * q.clamp(0.0, 1.0)).round() as usize;
    s[idx.min(s.len() - 1)]
}

/// Linear resample. Conditioning analysis uses it; the ECAPA binary keeps its own sinc resampler.
pub fn resample_linear(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 || x.is_empty() || from == to {
        return x.to_vec();
    }
    let n = ((x.len() as f64) * (to as f64) / (from as f64))
        .round()
        .max(1.0) as usize;
    let mut out = Vec::with_capacity(n);
    let last = x.len() - 1;
    for i in 0..n {
        let src = (i as f64) * (from as f64) / (to as f64);
        let j = (src.floor() as usize).min(last);
        let t = (src - j as f64) as f32;
        let a = x[j];
        let b = x[(j + 1).min(last)];
        out.push(a + (b - a) * t);
    }
    out
}

pub fn read_wav_mono(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    if channels == 0 {
        return Err("wav has zero channels".into());
    }
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32 / 32768.0))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        (hound::SampleFormat::Int, 24) => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 8_388_608.0))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        (hound::SampleFormat::Int, 32) => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 2_147_483_648.0))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        (fmt, bits) => return Err(format!("unsupported wav {fmt:?}/{bits}")),
    };
    let mono = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    Ok((sanitize(&mono), spec.sample_rate))
}

pub fn write_wav_s16(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    for &sample in samples {
        let quantized = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        writer.write_sample(quantized).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn canonical_join(path: &Path) -> PathBuf {
    let abs = absolute(path);
    let mut pending: Vec<OsString> = Vec::new();
    let mut cursor = abs.as_path();
    loop {
        if cursor.exists() {
            let mut full = std::fs::canonicalize(cursor).unwrap_or_else(|_| cursor.to_path_buf());
            for name in pending.iter().rev() {
                full.push(name);
            }
            return full;
        }
        if let Some(name) = cursor.file_name() {
            pending.push(name.to_os_string());
        }
        match cursor.parent() {
            Some(parent) if parent != cursor => cursor = parent,
            _ => return abs,
        }
    }
}

/// Audio, JSON reports, and weights stay outside the git tree. Consent clips stay in the clip store.
pub fn ensure_output_allowed(path: &Path) -> Result<(), String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo =
        std::fs::canonicalize(manifest.join("../../..")).unwrap_or_else(|_| manifest.to_path_buf());
    let fidelity = std::fs::canonicalize(manifest).unwrap_or_else(|_| manifest.to_path_buf());
    let name = path
        .file_name()
        .ok_or_else(|| "output path has no file name".to_string())?;
    let lower = name.to_string_lossy().to_ascii_lowercase();
    if lower.contains("secret")
        || lower.ends_with(".onnx")
        || lower.ends_with(".pt")
        || lower.ends_with(".ckpt")
        || lower.ends_with(".safetensors")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
    {
        return Err("refusing to write weights or secret-like names".into());
    }
    let full = canonical_join(path);
    if full.starts_with(&repo) || full.starts_with(&fidelity) {
        return Err(format!(
            "refusing to write inside the repo ({}); audio and weights stay local",
            full.display()
        ));
    }
    Ok(())
}
