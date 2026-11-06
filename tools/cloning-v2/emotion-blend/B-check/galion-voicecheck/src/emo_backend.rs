//! Pluggable emotion-classifier backend.
//!
//! Backends:
//! - `galion-emotion` — SUPERB 4-class + MSP A/V/D + proxies (legacy).
//! - `universal` — emotion2vec+ (9-class + 768-d emb) + EfficientAT (yelling/crying/laughter
//!   via Rust `ort` `galion-efficientat`) + LAION-CLAP zero-shot (love/seductive/logic).
//!   Batch path: `scorers/universal/py/` + EfficientAT binary. In-process Rust `ort` for
//!   emotion2vec+ remains a future drop-in behind this same trait.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::analysis::{load, run_logged};
use crate::emotion_match::{EmoVec, DEFAULT_WINDOW_S};
use crate::dsp::{read_mono, write_mono16};

/// Uniform classified-window record every backend must be able to produce (via EmoVec).
pub trait EmotionBackend: Send + Sync {
    fn id(&self) -> &str;
    fn score_files(&self, wavs: &[PathBuf], out_json: &Path) -> Result<Value>;
}

/// Default backend: existing galion-emotion fp32 binary.
pub struct GalionEmotionBackend {
    pub bin: PathBuf,
    pub threads: u32,
}

impl GalionEmotionBackend {
    pub fn new(root: &Path) -> Self {
        Self {
            bin: root.join("scorers/emotion/target/release/galion-emotion"),
            threads: 2,
        }
    }
}

impl EmotionBackend for GalionEmotionBackend {
    fn id(&self) -> &str { "galion-emotion" }
    fn score_files(&self, wavs: &[PathBuf], out_json: &Path) -> Result<Value> {
        if !self.bin.exists() {
            bail!("galion-emotion binary missing at {}", self.bin.display());
        }
        if out_json.exists() {
            return load(out_json);
        }
        if let Some(p) = out_json.parent() { std::fs::create_dir_all(p)?; }
        let mut cmd: Vec<String> = vec![
            "nice".into(), "-n".into(), "10".into(),
            self.bin.to_string_lossy().into(),
            "score".into(),
            "--threads".into(), self.threads.to_string(),
            "--compact".into(),
            "--out".into(), out_json.to_string_lossy().into(),
        ];
        for w in wavs { cmd.push(w.to_string_lossy().into()); }
        let log = out_json.with_extension("log");
        run_logged(&cmd, &log)?;
        load(out_json)
    }
}

pub struct UniversalBackend {
    pub root: PathBuf,
    pub threads: u32,
    pub skip_clap: bool,
}

impl UniversalBackend {
    pub fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf(), threads: 2, skip_clap: false }
    }
    fn py(&self) -> PathBuf { self.root.join("scorers/universal/venv/bin/python") }
    fn script(&self) -> PathBuf { self.root.join("scorers/universal/py/score_universal.py") }
}

impl EmotionBackend for UniversalBackend {
    fn id(&self) -> &str { "universal" }
    fn score_files(&self, wavs: &[PathBuf], out_json: &Path) -> Result<Value> {
        if !self.py().exists() || !self.script().exists() {
            bail!("universal scorer missing (need scorers/universal/venv + py/score_universal.py)");
        }
        if out_json.exists() { return load(out_json); }
        if let Some(p) = out_json.parent() { std::fs::create_dir_all(p)?; }
        let mut cmd: Vec<String> = vec![
            "nice".into(), "-n".into(), "10".into(),
            self.py().to_string_lossy().into(),
            self.script().to_string_lossy().into(),
            "--threads".into(), self.threads.to_string(),
            "--out".into(), out_json.to_string_lossy().into(),
        ];
        if self.skip_clap { cmd.push("--skip-clap".into()); }
        for w in wavs { cmd.push(w.to_string_lossy().into()); }
        let log = out_json.with_extension("log");
        run_logged(&cmd, &log)?;
        load(out_json)
    }
}

pub fn resolve(name: &str, root: &Path) -> Result<Box<dyn EmotionBackend>> {
    match name {
        "" | "galion-emotion" | "default" => Ok(Box::new(GalionEmotionBackend::new(root))),
        "universal" | "emotion2vec" | "emotion2vec+" => Ok(Box::new(UniversalBackend::new(root))),
        other => bail!("unknown emotion_backend {:?}; known: galion-emotion, universal", other),
    }
}

fn mem_avail_mb() -> u64 {
    std::fs::read_to_string("/proc/meminfo").ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("MemAvailable"))
            .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
        .unwrap_or(0) / 1024
}

/// Cut [start,end) seconds from a READ-ONLY source into `dest` (never modifies source).
pub fn cut_window(src: &Path, dest: &Path, start_s: f64, end_s: f64) -> Result<()> {
    let (x, sr) = read_mono(src)?;
    let s = ((start_s * sr as f64).round() as usize).min(x.len());
    let e = ((end_s * sr as f64).round() as usize).min(x.len()).max(s);
    if e <= s { bail!("empty window {}-{} on {}", start_s, end_s, src.display()); }
    if let Some(p) = dest.parent() { std::fs::create_dir_all(p)?; }
    write_mono16(dest, &x[s..e], sr)
}

/// Build contiguous windows covering the whole recording (last may be shorter, dropped if < 1.2 s).
pub fn plan_windows(duration_s: f64, window_s: f64) -> Vec<(f64, f64)> {
    let w = if window_s > 0.0 { window_s } else { DEFAULT_WINDOW_S };
    let mut out = Vec::new();
    let mut t = 0.0;
    while t + 1.2 <= duration_s + 1e-9 {
        let end = (t + w).min(duration_s);
        if end - t >= 1.2 { out.push((t, end)); }
        t += w;
    }
    if out.is_empty() && duration_s > 0.5 {
        out.push((0.0, duration_s));
    }
    out
}

pub fn wav_duration_s(path: &Path) -> Result<f64> {
    let (x, sr) = read_mono(path)?;
    Ok(x.len() as f64 / sr as f64)
}

/// Segment base into temp windows, score with backend, return EmoVec list. Source is never modified.
pub fn classify_base_windows(
    backend: &dyn EmotionBackend,
    base: &Path,
    work: &Path,
    window_s: f64,
    forced: Option<(f64, f64)>,
) -> Result<Vec<EmoVec>> {
    while mem_avail_mb() < 3000 {
        eprintln!("  [emotion] waiting for memory ({} MB avail)", mem_avail_mb());
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
    let dur = wav_duration_s(base)?;
    let mut spans = plan_windows(dur, window_s);
    if let Some((s, e)) = forced {
        // Ensure the forced joyful window is among the scored set (dedupe by start).
        if !spans.iter().any(|(a, b)| (*a - s).abs() < 0.05 && (*b - e).abs() < 0.05) {
            spans.push((s, e.min(dur)));
        }
    }
    let wdir = work.join("emo-windows");
    std::fs::create_dir_all(&wdir)?;
    let mut paths = Vec::new();
    let mut meta = Vec::new();
    for (i, (s, e)) in spans.iter().enumerate() {
        let dest = wdir.join(format!("w{:02}_{:.3}_{:.3}.wav", i, s, e));
        if !dest.exists() {
            cut_window(base, &dest, *s, *e)?;
        }
        paths.push(dest.clone());
        meta.push((format!("w{:02}", i), *s, *e));
    }
    let scored = work.join("base-windows-emo.json");
    let doc = backend.score_files(&paths, &scored)?;
    let files = doc["files"].as_array().context("backend json missing files[]")?;
    let mut out = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let (id, s, e) = meta.get(i).cloned().unwrap_or_else(|| (format!("w{}", i), 0.0, 0.0));
        if let Some(ev) = EmoVec::from_galion_file(f, &id, s, e) {
            out.push(ev);
        }
    }
    Ok(out)
}

/// Build EmoVec list from an already-scored galion-emotion JSON (clone lines).
pub fn lines_from_emo_json(emo: &Value, ids: &[String], wavs: &[PathBuf]) -> Vec<(String, EmoVec)> {
    let mut by_path = std::collections::BTreeMap::new();
    if let Some(arr) = emo["files"].as_array() {
        for f in arr {
            if let Some(p) = f["file"].as_str() {
                by_path.insert(p.to_string(), f);
            }
        }
    }
    let mut out = Vec::new();
    for (id, w) in ids.iter().zip(wavs) {
        let p = w.to_string_lossy().to_string();
        let canon = std::fs::canonicalize(w).ok().map(|c| c.to_string_lossy().to_string());
        let f = by_path.get(&p).or_else(|| canon.as_ref().and_then(|c| by_path.get(c))).copied();
        if let Some(f) = f {
            if let Some(ev) = EmoVec::from_galion_file(f, id, 0.0, f["duration_s"].as_f64().unwrap_or(0.0)) {
                out.push((id.clone(), ev));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_windows_covers() {
        let w = plan_windows(17.4, 2.5);
        assert!(w.len() >= 6);
        assert!((w[0].0 - 0.0).abs() < 1e-9);
        assert!(w.last().unwrap().1 <= 17.4 + 1e-9);
    }

    #[test]
    fn resolve_default() {
        let b = resolve("galion-emotion", Path::new("/tmp")).unwrap();
        assert_eq!(b.id(), "galion-emotion");
        let u = resolve("universal", Path::new("/tmp")).unwrap();
        assert_eq!(u.id(), "universal");
        assert!(resolve("nope-model", Path::new("/tmp")).is_err());
    }
}
