use crate::scorers::{
    extract_ecapa_sim, extract_emotion_distance, is_ready, load_scorer, run_pair_scorer, ScorerBin,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub const DEFAULT_CBX: &str =
    "/workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx";

#[derive(Clone, Debug)]
pub struct BestOfArgs {
    pub cbx: PathBuf,
    pub voice: String,
    pub text: String,
    pub reference: PathBuf,
    pub out: PathBuf,
    pub n: usize,
    pub repetition_penalty: f32,
    pub dtype: String,
    pub intra_threads: usize,
    pub work_dir: PathBuf,
    /// Weight on (1 - emotion_distance). Emotion is top priority.
    pub w_emotion: f64,
    pub w_ecapa: f64,
}

fn speak_one(a: &BestOfArgs, cand_path: &Path) -> Result<f64> {
    let t0 = Instant::now();
    let status = Command::new("nice")
        .args(["-n", "10"])
        .arg(&a.cbx)
        .args([
            "--intra-threads",
            &a.intra_threads.to_string(),
            "speak",
            "--dtype",
            &a.dtype,
            "--ep",
            "cpu",
            "--voice",
            &a.voice,
            "--text",
            &a.text,
            "--repetition-penalty",
            &a.repetition_penalty.to_string(),
            "--out-wav",
            &cand_path.display().to_string(),
        ])
        .status()
        .context("spawn cbx")?;
    if !status.success() {
        bail!("cbx speak failed: {status}");
    }
    Ok(t0.elapsed().as_secs_f64() * 1000.0)
}

fn composite(emotion_dist: Option<f64>, ecapa: Option<f64>, w_e: f64, w_s: f64) -> f64 {
    // Higher is better. Emotion distance → similarity proxy (1 - dist) with priority weight.
    let e = emotion_dist.map(|d| 1.0 - d).unwrap_or(0.0);
    let s = ecapa.unwrap_or(0.0);
    w_e * e + w_s * s
}

pub fn run_best_of_n(a: &BestOfArgs) -> Result<Value> {
    let t_all = Instant::now();
    fs::create_dir_all(&a.work_dir)?;
    let emotion: Option<ScorerBin> = if is_ready("emotion") {
        Some(load_scorer("emotion")?)
    } else {
        None
    };
    let ecapa: Option<ScorerBin> = if is_ready("ecapa") {
        Some(load_scorer("ecapa")?)
    } else {
        None
    };
    if emotion.is_none() && ecapa.is_none() {
        bail!("no Rust scorers READY yet (need emotion and/or ecapa under {SCORERS})", SCORERS = crate::scorers::SCORERS_ROOT);
    }

    let mut candidates = Vec::new();
    let mut best_idx = 0usize;
    let mut best_score = f64::NEG_INFINITY;

    for i in 0..a.n {
        let cand = a.work_dir.join(format!("cand_{i:02}.wav"));
        let synth_ms = speak_one(a, &cand)?;
        let mut emotion_dist = None;
        let mut emotion_raw = json!(null);
        let mut emotion_ms = 0.0;
        if let Some(ref sc) = emotion {
            let (v, ms) = run_pair_scorer(sc, &cand, &a.reference)?;
            emotion_ms = ms;
            emotion_raw = v.clone();
            emotion_dist = extract_emotion_distance(&v);
        }
        let mut ecapa_sim = None;
        let mut ecapa_raw = json!(null);
        let mut ecapa_ms = 0.0;
        if let Some(ref sc) = ecapa {
            let (v, ms) = run_pair_scorer(sc, &cand, &a.reference)?;
            ecapa_ms = ms;
            ecapa_raw = v.clone();
            ecapa_sim = extract_ecapa_sim(&v);
        }
        let score = composite(emotion_dist, ecapa_sim, a.w_emotion, a.w_ecapa);
        let entry = json!({
            "i": i,
            "path": cand.display().to_string(),
            "synth_ms": synth_ms,
            "emotion_distance": emotion_dist,
            "emotion_ms": emotion_ms,
            "emotion_raw": emotion_raw,
            "ecapa": ecapa_sim,
            "ecapa_ms": ecapa_ms,
            "ecapa_raw": ecapa_raw,
            "composite": score,
        });
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
        candidates.push(entry);
    }

    let best_src = a.work_dir.join(format!("cand_{best_idx:02}.wav"));
    fs::copy(&best_src, &a.out).with_context(|| format!("copy best to {}", a.out.display()))?;

    Ok(json!({
        "voice": a.voice,
        "text": a.text,
        "n": a.n,
        "repetition_penalty": a.repetition_penalty,
        "reference": a.reference.display().to_string(),
        "out": a.out.display().to_string(),
        "w_emotion": a.w_emotion,
        "w_ecapa": a.w_ecapa,
        "best_idx": best_idx,
        "best_composite": best_score,
        "candidates": candidates,
        "scorers": {
            "emotion_ready": emotion.is_some(),
            "ecapa_ready": ecapa.is_some(),
        },
        "elapsed_ms": t_all.elapsed().as_secs_f64() * 1000.0,
    }))
}
