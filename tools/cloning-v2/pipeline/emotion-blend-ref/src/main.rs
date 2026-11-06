//! Global expressive reference selection. No voice names, labels, or target emotions are hardcoded.
use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    meta: PathBuf,
    /// Rust fp32 galion-emotion executable; all window inference is one sequential process.
    #[arg(long)]
    scorer: PathBuf,
    /// Reuse a previous score file only if its file paths match this run's windows exactly.
    #[arg(long)]
    scores: Option<PathBuf>,
}
fn read(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path)?;
    let s = r.spec();
    if s.channels == 0 || s.sample_rate == 0 {
        bail!("invalid WAV format");
    }
    let v: Vec<f32> = match s.sample_format {
        hound::SampleFormat::Float => r
            .samples::<f32>()
            .map(|x| x.map_err(Into::into))
            .collect::<Result<_>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(s.bits_per_sample as i32 - 1);
            r.samples::<i32>()
                .map(|x| x.map(|v| v as f32 / scale).map_err(Into::into))
                .collect::<Result<_>>()?
        }
    };
    if v.iter().any(|x| !x.is_finite()) {
        bail!("nonfinite reference samples");
    }
    let mono = v
        .chunks_exact(s.channels as usize)
        .map(|c| c.iter().sum::<f32>() / s.channels as f32)
        .collect();
    Ok((mono, s.sample_rate))
}
fn write(path: &Path, x: &[f32], sr: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // create_new prevents overwrites, including symlinks/hardlinks to originals.
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("refuse overwrite: {}", path.display()))?;
    let mut w = hound::WavWriter::new(
        f,
        hound::WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    for &s in x {
        w.write_sample(s)?;
    }
    w.finalize()?;
    Ok(())
}
fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
/// Farthest pair first, then farthest-first diversity, NOT maximum arousal.
/// A centroid/medoid consumes a scarce slot and can suppress minority emotions.
fn select(features: &[Vec<f64>], k: usize) -> Vec<usize> {
    if features.is_empty() || k == 0 {
        return vec![];
    }
    let mut chosen = vec![0];
    if features.len() > 1 && k > 1 {
        let mut pair = (0, 1);
        let mut best = -1.0;
        for i in 0..features.len() {
            for j in i + 1..features.len() {
                let d = distance(&features[i], &features[j]);
                if d > best {
                    best = d;
                    pair = (i, j);
                }
            }
        }
        chosen = vec![pair.0, pair.1];
    }
    while chosen.len() < k.min(features.len()) {
        let next = (0..features.len())
            .filter(|i| !chosen.contains(i))
            .max_by(|&a, &b| {
                let d = |i: usize| {
                    chosen
                        .iter()
                        .map(|&j| distance(&features[i], &features[j]))
                        .fold(f64::INFINITY, f64::min)
                };
                d(a).total_cmp(&d(b)).then(b.cmp(&a))
            })
            .unwrap();
        chosen.push(next);
    }
    chosen.sort_unstable();
    chosen
}
fn montage(x: &[f32], sr: u32, ranges: &[(usize, usize)], chosen: &[usize]) -> Vec<f32> {
    let mut out = Vec::new();
    for &i in chosen {
        let (a, b) = ranges[i];
        let end = b.min(a + 2 * sr as usize);
        let mut part = x[a..end].to_vec();
        // Edge-only 5ms ramps avoid clicks; no segment loudness normalisation or dynamic compression.
        let fade = (sr as usize / 200).min(part.len() / 2);
        for j in 0..fade {
            let gain = j as f32 / fade as f32;
            part[j] *= gain;
            let n = part.len();
            part[n - 1 - j] *= gain;
        }
        out.extend(part);
    }
    out
}
fn feature(f: &Value) -> Result<Vec<f64>> {
    let mut v = Vec::new();
    for key in ["neu", "hap", "ang", "sad"] {
        v.push(
            f["ser"]["probs"][key]
                .as_f64()
                .context("missing fp32 SER probabilities")?,
        );
    }
    // SER is uncertainty, not truth; dimensional/prosodic features also contribute.
    for key in ["arousal", "valence", "dominance"] {
        v.push(0.5 * f["avd"][key].as_f64().context("missing AVD")?);
    }
    for (key, scale) in [
        ("f0_sd_st", 12.0),
        ("energy_dr_db", 60.0),
        ("speech_rate_syl_s", 12.0),
    ] {
        v.push(0.25 * f["proxies"][key].as_f64().unwrap_or(0.0) / scale);
    }
    if v.iter().any(|x| !x.is_finite()) {
        bail!("invalid scoring features");
    }
    Ok(v)
}
fn main() -> Result<()> {
    let a = Args::parse();
    if a.output.exists() || a.meta.exists() {
        bail!("output/meta already exist; use a new run directory");
    }
    let (x, sr) = read(&a.input)?;
    if x.len() < (sr as usize / 2) {
        bail!("reference shorter than 0.5s");
    }
    let work = a.meta.with_extension("windows");
    std::fs::create_dir_all(&work)?;
    let n = ((x.len() as f64 / sr as f64 / 2.5).round() as usize).max(1);
    if n > 240 {
        bail!("reference too long (>~10min), select a consented speech excerpt first");
    }
    let ranges: Vec<_> = (0..n)
        .map(|i| (i * x.len() / n, (i + 1) * x.len() / n))
        .collect();
    let mut paths = Vec::new();
    for (i, &(start, end)) in ranges.iter().enumerate() {
        let p = work.join(format!("window-{i:04}.wav"));
        write(&p, &x[start..end], sr)?;
        paths.push(p.canonicalize()?);
    }
    let score_path = if let Some(p) = a.scores {
        p
    } else {
        let p = work.join("scores.json");
        let status = Command::new(&a.scorer)
            .args(["score", "--threads", "2", "--out"])
            .arg(&p)
            .args(&paths)
            .status()?;
        if !status.success() {
            bail!("fp32 Rust emotion scorer failed");
        }
        p
    };
    let scores: Value = serde_json::from_slice(&std::fs::read(&score_path)?)?;
    let files = scores["files"].as_array().context("scorer omitted files")?;
    let mut valid = Vec::new();
    let mut features = Vec::new();
    let mut details = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        let s = files
            .iter()
            .find(|f| f["file"].as_str().map(Path::new) == Some(p.as_path()))
            .context("window score mismatch")?;
        if s["proxies"]["voiced_frac"].as_f64().unwrap_or(0.0) < 0.1
            || s["proxies"]["rms_dbfs"].as_f64().unwrap_or(-100.0) < -55.0
        {
            continue;
        }
        valid.push(i);
        features.push(feature(s)?);
        details.push(s.clone());
    }
    if valid.is_empty() {
        bail!("no speech windows passed eligibility; no reference produced");
    }
    let selected_local = select(&features, 3);
    let selected: Vec<_> = selected_local.iter().map(|&i| valid[i]).collect();
    let y = montage(&x, sr, &ranges, &selected);
    write(&a.output, &y, sr)?;
    let metadata = json!({"schema":"galion-blend-ref/1","input":a.input.canonicalize()?,"output":a.output,
        "algorithm":"farthest pair + farthest-first SER/AVD/prosody diversity; chronological 2s excerpts; edge ramps only",
        "sample_rate":sr,"duration_s":y.len() as f64/sr as f64,"scorer":"galion-emotion fp32, threads=2",
        "speaker_policy":"caller MUST retain original speaker_embeddings and speaker_features; use montage only for audio_features/audio_tokens",
        "eligible_window_indices":valid,"selected_window_indices":selected,
        "segments":selected.iter().map(|&i|json!({"window_index":i,"start_s":ranges[i].0 as f64/sr as f64,
            "end_s":ranges[i].1.min(ranges[i].0+2*sr as usize) as f64/sr as f64})).collect::<Vec<_>>(),
        "eligible_scores":details,"promotion":"OFF BY DEFAULT; requires per-voice WER/ECAPA validation"});
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&a.meta)?
        .write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
    println!("{}", serde_json::to_string_pretty(&metadata)?);
    Ok(())
}
use std::io::Write;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diversity_is_not_maximum_arousal() {
        let f = vec![
            vec![1., 0., 0.],
            vec![0.99, 0.01, 0.],
            vec![0., 1., 0.],
            vec![0., 0., 1.],
        ];
        let s = select(&f, 3);
        assert_eq!(s.len(), 3);
        assert!(s.contains(&2) && s.contains(&3));
    }
    #[test]
    fn intermediate_medoid_does_not_crowd_out_minority_style() {
        let f = vec![
            vec![0.96, 0.02, 0.02],
            vec![0.56, 0.36, 0.08],
            vec![0.02, 0.02, 0.96],
            vec![0.65, 0.02, 0.33],
        ];
        assert_eq!(select(&f, 3), vec![0, 1, 2]);
    }
    #[test]
    fn selection_deterministic_and_unique() {
        let f = vec![vec![1.]; 8];
        let s = select(&f, 3);
        assert_eq!(s, select(&f, 3));
        assert_eq!(s, vec![0, 1, 2]);
    }
    #[test]
    fn empty_and_small_inputs() {
        assert!(select(&[], 3).is_empty());
        assert_eq!(select(&[vec![0.]], 3), vec![0]);
    }
    #[test]
    fn montage_is_bounded_and_keeps_interior_dynamics() {
        let x: Vec<f32> = (0..2000)
            .map(|i| if i < 1000 { 0.1 } else { 0.8 })
            .collect();
        let y = montage(&x, 100, &[(0, 1000), (1000, 2000)], &[0, 1]);
        assert_eq!(y.len(), 400);
        assert_eq!(y[100], 0.1);
        assert_eq!(y[300], 0.8);
    }
    #[test]
    fn missing_scores_fail_closed() {
        assert!(feature(&json!({})).is_err());
    }
}
