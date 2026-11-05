// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Invoke galion-emotion / galion-ecapa / galion-wer and build line deltas.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::gate::GateScores;
use crate::paths::Paths;
use crate::profile::{line_delta, BaseProsody, LineDelta};

#[derive(Debug, Clone)]
pub struct AnalysisBundle {
    pub emotion_json: PathBuf,
    pub ecapa_json: PathBuf,
    pub wer_json: PathBuf,
    pub lines: Vec<LineDelta>,
    pub scores: GateScores,
    pub emotion_distance: Option<f64>,
    pub label_counts: Value,
}

fn run_nice(cmd: &str, args: &[&str], out_hint: &str, log_path: &Path) -> Result<()> {
    let log = std::fs::File::create(log_path)
        .with_context(|| format!("create log {}", log_path.display()))?;
    let status = Command::new("nice")
        .args(["-n", "10"])
        .arg(cmd)
        .args(args)
        .stdout(log.try_clone()?)
        .stderr(log)
        .status()
        .with_context(|| format!("spawn {cmd}"))?;
    if !status.success() {
        bail!("{out_hint}: {cmd} failed with {:?} (see {})", status.code(), log_path.display());
    }
    Ok(())
}

pub fn score_wavs(
    paths: &Paths,
    wavs: &[PathBuf],
    text_dir: &Path,
    ref_wav: &Path,
    work_dir: &Path,
    base: &BaseProsody,
) -> Result<AnalysisBundle> {
    fs::create_dir_all(work_dir)?;
    let emotion_json = work_dir.join("emotion.json");
    let ecapa_json = work_dir.join("ecapa.json");
    let wer_json = work_dir.join("wer.json");

    // Emotion (fp32) vs base recording reference when available.
    let mut emo_args: Vec<String> = vec![
        "score".into(),
        "--threads".into(),
        "2".into(),
        "--out".into(),
        emotion_json.display().to_string(),
    ];
    if ref_wav.is_file() {
        emo_args.push("--ref".into());
        emo_args.push(ref_wav.display().to_string());
    }
    for w in wavs {
        emo_args.push(w.display().to_string());
    }
    let emo_refs: Vec<&str> = emo_args.iter().map(|s| s.as_str()).collect();
    run_nice(
        &paths.emotion.display().to_string(),
        &emo_refs,
        "galion-emotion",
        &work_dir.join("emotion.log"),
    )?;

    // ECAPA
    let mut ecapa_args: Vec<String> = vec![
        "--ref".into(),
        ref_wav.display().to_string(),
        "--threads".into(),
        "2".into(),
        "--out".into(),
        ecapa_json.display().to_string(),
    ];
    for w in wavs {
        ecapa_args.push(w.display().to_string());
    }
    let ecapa_refs: Vec<&str> = ecapa_args.iter().map(|s| s.as_str()).collect();
    run_nice(
        &paths.ecapa.display().to_string(),
        &ecapa_refs,
        "galion-ecapa",
        &work_dir.join("ecapa.log"),
    )?;

    // WER
    let mut wer_args: Vec<String> = vec![
        "--text-dir".into(),
        text_dir.display().to_string(),
        "--jobs".into(),
        "2".into(),
        "--out".into(),
        wer_json.display().to_string(),
    ];
    for w in wavs {
        wer_args.push(w.display().to_string());
    }
    let wer_refs: Vec<&str> = wer_args.iter().map(|s| s.as_str()).collect();
    run_nice(
        &paths.wer.display().to_string(),
        &wer_refs,
        "galion-wer",
        &work_dir.join("wer.log"),
    )?;

    parse_bundle(&emotion_json, &ecapa_json, &wer_json, base)
}

pub fn parse_bundle(
    emotion_json: &Path,
    ecapa_json: &Path,
    wer_json: &Path,
    base: &BaseProsody,
) -> Result<AnalysisBundle> {
    let emo: Value = serde_json::from_str(&fs::read_to_string(emotion_json)?)?;
    let ecapa: Value = serde_json::from_str(&fs::read_to_string(ecapa_json)?)?;
    let wer: Value = serde_json::from_str(&fs::read_to_string(wer_json)?)?;

    let ecapa_mean = ecapa
        .get("mean")
        .and_then(|x| x.as_f64())
        .context("ecapa.mean")?;
    let wer_mean = wer
        .pointer("/summary/mean")
        .and_then(|x| x.as_f64())
        .context("wer.summary.mean")?;

    let mut lines = Vec::new();
    if let Some(files) = emo.get("files").and_then(|x| x.as_array()) {
        for f in files {
            let wav = f.get("file").and_then(|x| x.as_str()).unwrap_or("");
            let stem = Path::new(wav)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("line");
            lines.push(line_delta(stem, f, base)?);
        }
    }

    let emotion_distance = emo
        .pointer("/summary/mean_emotion_distance")
        .and_then(|x| x.as_f64());
    let label_counts = emo
        .pointer("/summary/label_counts")
        .cloned()
        .unwrap_or(Value::Null);

    Ok(AnalysisBundle {
        emotion_json: emotion_json.to_path_buf(),
        ecapa_json: ecapa_json.to_path_buf(),
        wer_json: wer_json.to_path_buf(),
        lines,
        scores: GateScores {
            wer_mean,
            ecapa_mean,
        },
        emotion_distance,
        label_counts,
    })
}

/// Reuse already-scored merge-v2 JSON when present (fast dry-run path).
pub fn try_load_merge_scores(
    persona: &str,
    base: &BaseProsody,
) -> Result<Option<AnalysisBundle>> {
    let prefix = match persona {
        "samantha" => "sam_merge_v2",
        "jarvis" => "jar_merge_v2",
        _ => return Ok(None),
    };
    let dir = PathBuf::from("/workspace/swarm/voice-v2-sprint/cloning-v2/merge-v2/scores");
    let emotion_json = dir.join(format!("{prefix}_emo.json"));
    let ecapa_json = dir.join(format!("{prefix}_ecapa.json"));
    let wer_json = dir.join(format!("{prefix}_wer.json"));
    if emotion_json.is_file() && ecapa_json.is_file() && wer_json.is_file() {
        return Ok(Some(parse_bundle(
            &emotion_json,
            &ecapa_json,
            &wer_json,
            base,
        )?));
    }
    Ok(None)
}
