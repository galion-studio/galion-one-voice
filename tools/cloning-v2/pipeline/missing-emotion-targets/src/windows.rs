//! Window planning and read-only original cutting.
//! `plan_windows` matches `scorers/universal/py/universal_match.py:plan_windows`.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub start_s: f64,
    pub end_s: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowMeta {
    pub id: String,
    pub path: String,
    pub start_s: f64,
    pub end_s: f64,
}

/// Same loop as Python: t = 0; while t + 1.2 <= duration; end = min(t+window, dur);
/// keep if end - t >= 1.2; t += window.
pub fn plan_windows(duration_s: f64, window_s: f64) -> Vec<Span> {
    let mut out = Vec::new();
    let mut t = 0.0;
    while t + 1.2 <= duration_s + 1e-9 {
        let end = (t + window_s).min(duration_s);
        if end - t >= 1.2 {
            out.push(Span {
                start_s: t,
                end_s: end,
            });
        }
        t += window_s;
    }
    out
}

pub fn parse_window_spec(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once('-')?;
    let start: f64 = a.trim().parse().ok()?;
    let end: f64 = b.trim().parse().ok()?;
    if end > start && start >= 0.0 {
        Some((start, end))
    } else {
        None
    }
}

pub fn read_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = r.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        bail!("invalid WAV {}", path.display());
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
            r.samples::<i32>()
                .map(|x| x.map(|v| v as f32 / scale))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
    };
    if samples.iter().any(|x| !x.is_finite()) {
        bail!("nonfinite samples in {}", path.display());
    }
    let mono = samples
        .chunks_exact(spec.channels as usize)
        .map(|c| c.iter().sum::<f32>() / spec.channels as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

pub fn duration_s(path: &Path) -> Result<f64> {
    let (mono, sr) = read_mono(path)?;
    Ok(mono.len() as f64 / sr as f64)
}

/// Write a mono f32 cut. `create_new` so originals and existing files are never overwritten.
pub fn cut_window(src: &Path, dest: &Path, start_s: f64, end_s: f64) -> Result<()> {
    if dest.exists() {
        bail!("refuse overwrite: {}", dest.display());
    }
    if is_protected_original(dest) {
        bail!("refuse write into protected original path {}", dest.display());
    }
    let (mono, sr) = read_mono(src)?;
    let start = ((start_s * sr as f64).round() as usize).min(mono.len());
    let end = ((end_s * sr as f64).round() as usize).min(mono.len()).max(start);
    if end <= start {
        bail!("empty window {start_s}-{end_s} from {}", src.display());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .with_context(|| format!("create {}", dest.display()))?;
    let mut w = hound::WavWriter::new(
        f,
        hound::WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    for &s in &mono[start..end] {
        w.write_sample(s)?;
    }
    w.finalize()?;
    Ok(())
}

pub fn is_protected_original(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/clip-store-own/") && (s.ends_with("/original.wav") || s.ends_with("/reference.wav"))
    {
        return true;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, "Samantha.wav" | "Jarvis.wav" | "original.wav" | "reference.wav")
        && (s.contains("clip-store-own") || s.contains("hf-samantha") || s.contains("hf-jarvis"))
}

pub fn window_filename(i: usize, start_s: f64, end_s: f64) -> String {
    format!("w{i:02}_{start_s:.3}_{end_s:.3}.wav")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samantha_duration_makes_seven_windows() {
        let w = plan_windows(17.40718820861678, 2.5);
        assert_eq!(w.len(), 7);
        assert_eq!(w[0], Span { start_s: 0.0, end_s: 2.5 });
        assert!((w[6].start_s - 15.0).abs() < 1e-12);
        assert!((w[6].end_s - 17.40718820861678).abs() < 1e-12);
    }

    #[test]
    fn jarvis_duration_makes_six_windows() {
        let w = plan_windows(15.9520625, 2.5);
        assert_eq!(w.len(), 6);
        assert!((w[5].start_s - 12.5).abs() < 1e-12);
        assert!((w[5].end_s - 15.0).abs() < 1e-12);
        // next candidate starts at 15.0; 15.0 + 1.2 = 16.2 > 15.952, so no w06
    }

    #[test]
    fn short_audio_yields_no_window() {
        assert!(plan_windows(1.0, 2.5).is_empty());
    }

    #[test]
    fn parse_window_spec_ok() {
        assert_eq!(parse_window_spec("9.947-12.434"), Some((9.947, 12.434)));
        assert!(parse_window_spec("12-10").is_none());
    }

    #[test]
    fn protected_original_paths() {
        assert!(is_protected_original(Path::new(
            "/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav"
        )));
        assert!(is_protected_original(Path::new(
            "/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/reference.wav"
        )));
        assert!(!is_protected_original(Path::new(
            "/tmp/missing-emotion-targets/windows/w00_0.000_2.500.wav"
        )));
    }
}
