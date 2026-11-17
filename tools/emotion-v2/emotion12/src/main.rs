// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-emotion12 — thin glue for the Emotion V2 gate.
//!
//!   galion-emotion12 score [opts] --out out.json clip.wav...
//!   galion-emotion12 bench [opts] --corpus crema-d|ravdess --root DIR --out out.json
//!   galion-emotion12 coverage [--emotions FILE]
//!
//! opts: --emotions FILE (default tools/emotion-v2/emotion12/gate-emotions.toml, 14 emotions), --model base|large,
//!       --models-dir DIR (default $EMOTION_V2_MODELS_DIR or G:\galion\models\emotion-v2),
//!       --device auto|cpu|cuda (auto = CUDA only when present), --threads N,
//!       --raw (base export: skip FunASR's waveform layer-norm; default applies it).
//! Needs ORT_DYLIB_PATH (onnxruntime 1.22; the GPU build for CUDA).
//! Does not download, does not change the 0.83 gate, never reads own-voice audio
//! unless a caller passes it to `score`.
mod audio;
mod config;
mod model;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

const GATE: f64 = 0.83; // reported against, never changed
const W_CAT: f64 = 0.70;
const W_EMB: f64 = 0.30;

struct Opts {
    emotions: String,
    model: String,
    models_dir: PathBuf,
    device: String,
    threads: usize,
    out: Option<PathBuf>,
    corpus: Option<String>,
    root: Option<PathBuf>,
    limit: usize,
    raw: bool,
    files: Vec<String>,
}

fn parse(args: &[String]) -> Result<Opts> {
    let default_models = std::env::var("EMOTION_V2_MODELS_DIR").unwrap_or_else(|_| r"G:\galion\models\emotion-v2".into());
    let mut o = Opts {
        emotions: "tools/emotion-v2/emotion12/gate-emotions.toml".into(),
        model: "base".into(),
        models_dir: default_models.into(),
        device: "auto".into(),
        threads: 4,
        out: None,
        corpus: None,
        root: None,
        limit: 0,
        raw: false,
        files: vec![],
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || -> Result<String> {
            i += 1;
            args.get(i).cloned().with_context(|| format!("{a} needs a value"))
        };
        match a {
            "--emotions" => o.emotions = val()?,
            "--model" => o.model = val()?,
            "--models-dir" => o.models_dir = val()?.into(),
            "--device" => o.device = val()?,
            "--threads" => o.threads = val()?.parse()?,
            "--out" => o.out = Some(val()?.into()),
            "--corpus" => o.corpus = Some(val()?),
            "--root" => o.root = Some(val()?.into()),
            "--limit" => o.limit = val()?.parse()?,
            "--raw" => o.raw = true,
            _ if a.starts_with("--") => bail!("unknown option {a}"),
            _ => o.files.push(a.to_string()),
        }
        i += 1;
    }
    Ok(o)
}

fn load16(path: &str) -> Result<Vec<f32>> {
    let a = audio::load_wav(path)?;
    Ok(audio::resample(&a.samples, a.sr, 16000))
}

fn unit(v: &[f32]) -> Vec<f64> {
    let n = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt().max(1e-12);
    v.iter().map(|x| *x as f64 / n).collect()
}

fn write(out: &Option<PathBuf>, v: &Value) -> Result<()> {
    let s = serde_json::to_string_pretty(v)?;
    match out {
        Some(p) => {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).ok();
            }
            std::fs::write(p, s)?;
        }
        None => println!("{s}"),
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { bail!("usage: galion-emotion12 score|bench|coverage ...") };
    let o = parse(&args[1..])?;
    let emo = config::Emotions::load(&o.emotions)?;
    let labels: Vec<String> = model::LABELS.iter().map(|s| s.to_string()).collect();
    match cmd.as_str() {
        "coverage" => {
            for (id, kind) in emo.coverage(&labels) {
                println!("{id}\t{kind}");
            }
            Ok(())
        }
        "score" => {
            model::init_ort()?;
            let mut s = model::Scorer::new(&o.models_dir, &o.model, o.threads, &o.device)?;
            s.normalize = !o.raw;
            let mut rows = vec![];
            for f in &o.files {
                let out = s.score(&load16(f)?)?;
                rows.push(json!({
                    "file": f,
                    "posterior": emo.project(&out.labels, &out.probs)?,
                    "top": emo.predict(&out.labels, &out.probs),
                    "model_probs": out.probs,
                    "embedding": out.embedding,
                }));
            }
            write(&o.out, &json!({
                "schema": "galion-emotion12/1",
                "model": format!("emotion2vec+ {}", s.model),
                "device": s.device,
                "emotions_config": o.emotions,
                "labels": emo.ids,
                "model_labels": labels,
                "files": rows,
            }))
        }
        "bench" => bench(&o, &emo, &labels),
        other => bail!("unknown command {other}"),
    }
}

/// (path, true emotion id, actor) for a public acted-emotion corpus.
fn corpus(name: &str, root: &Path) -> Result<Vec<(String, String, String)>> {
    let mut rows = vec![];
    match name {
        "crema-d" => {
            let map = BTreeMap::from([("ANG", "angry"), ("DIS", "disgusted"), ("FEA", "fearful"), ("HAP", "happy"), ("NEU", "neutral"), ("SAD", "sad")]);
            for e in std::fs::read_dir(root.join("AudioWAV"))? {
                let p = e?.path();
                let stem = p.file_stem().unwrap().to_string_lossy().to_string();
                let parts: Vec<&str> = stem.split('_').collect();
                if parts.len() == 4 && p.extension().map_or(false, |x| x == "wav") {
                    if let Some(l) = map.get(parts[2]) {
                        rows.push((p.to_string_lossy().into(), l.to_string(), parts[0].to_string()));
                    }
                }
            }
        }
        "ravdess" => {
            let map = BTreeMap::from([("01", "neutral"), ("02", "calm"), ("03", "happy"), ("04", "sad"), ("05", "angry"), ("06", "fearful"), ("07", "disgusted"), ("08", "surprised")]);
            for actor in std::fs::read_dir(root)? {
                let dir = actor?.path();
                if !dir.is_dir() { continue; }
                for e in std::fs::read_dir(&dir)? {
                    let p = e?.path();
                    let stem = p.file_stem().unwrap().to_string_lossy().to_string();
                    let parts: Vec<&str> = stem.split('-').collect();
                    if parts.len() == 7 && parts[1] == "01" {
                        rows.push((p.to_string_lossy().into(), map[parts[2]].to_string(), parts[6].to_string()));
                    }
                }
            }
        }
        _ => bail!("unknown corpus {name}"),
    }
    rows.sort();
    Ok(rows)
}

fn bench(o: &Opts, emo: &config::Emotions, labels: &[String]) -> Result<()> {
    let name = o.corpus.as_deref().context("--corpus")?;
    let mut clips = corpus(name, o.root.as_deref().context("--root")?)?;
    if o.limit > 0 && clips.len() > o.limit {
        let step = clips.len() / o.limit;
        clips = clips.into_iter().step_by(step).collect();
    }
    model::init_ort()?;
    let mut s = model::Scorer::new(&o.models_dir, &o.model, o.threads, &o.device)?;
    s.normalize = !o.raw;
    let t0 = Instant::now();
    let mut post = vec![];
    let mut embs = vec![];
    let mut preds = vec![];
    for (n, (path, _, _)) in clips.iter().enumerate() {
        let out = s.score(&load16(path)?)?;
        post.push(emo.project(&out.labels, &out.probs)?);
        preds.push(emo.predict(&out.labels, &out.probs).unwrap_or_default());
        embs.push(unit(&out.embedding));
        if n % 500 == 0 {
            eprintln!("{name}: {n}/{} {:.0}s", clips.len(), t0.elapsed().as_secs_f64());
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    // accuracy
    let present: Vec<String> = emo.ids.iter().filter(|e| clips.iter().any(|c| &c.1 == *e)).cloned().collect();
    let mut conf: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (c, p) in clips.iter().zip(&preds) {
        *conf.entry(c.1.clone()).or_default().entry(p.clone()).or_default() += 1;
    }
    let mut recall = BTreeMap::new();
    let mut correct = 0usize;
    for e in &present {
        let row = &conf[e];
        let tot: usize = row.values().sum();
        let ok = *row.get(e).unwrap_or(&0);
        correct += ok;
        recall.insert(e.clone(), ok as f64 / tot as f64);
    }
    let wa = correct as f64 / clips.len() as f64;
    let ua = recall.values().sum::<f64>() / recall.len() as f64;
    // gate formula on public clips: even/odd actor split, both folds.
    let mut actors: Vec<String> = clips.iter().map(|c| c.2.clone()).collect();
    actors.sort();
    actors.dedup();
    let k = emo.ids.len();
    let mut same: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut other: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for fold in 0..2 {
        let is_ref = |a: &String| actors.iter().position(|x| x == a).unwrap() % 2 == fold;
        let mut refs: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
        for e in &present {
            let idx: Vec<usize> = (0..clips.len()).filter(|&i| &clips[i].1 == e && is_ref(&clips[i].2)).collect();
            if idx.is_empty() { continue; }
            let p: Vec<f64> = (0..k).map(|j| idx.iter().map(|&i| post[i][j]).sum::<f64>() / idx.len() as f64).collect();
            let d = embs[idx[0]].len();
            let m: Vec<f32> = (0..d).map(|j| (idx.iter().map(|&i| embs[i][j]).sum::<f64>() / idx.len() as f64) as f32).collect();
            refs.insert(e.clone(), (p, unit(&m)));
        }
        for i in 0..clips.len() {
            if is_ref(&clips[i].2) || !refs.contains_key(&clips[i].1) { continue; }
            let score = |r: &(Vec<f64>, Vec<f64>)| {
                let cat: f64 = post[i].iter().zip(&r.0).map(|(a, b)| a.min(*b)).sum();
                let sim: f64 = embs[i].iter().zip(&r.1).map(|(a, b)| a * b).sum();
                W_CAT * cat + W_EMB * sim
            };
            let mine = score(&refs[&clips[i].1]);
            let best_other = refs.iter().filter(|(e, _)| **e != clips[i].1).map(|(_, r)| score(r)).fold(f64::MIN, f64::max);
            same.entry(clips[i].1.clone()).or_default().push(mine);
            other.entry(clips[i].1.clone()).or_default().push(best_other);
        }
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let r4 = |x: f64| (x * 10000.0).round() / 10000.0;
    let mut gate = serde_json::Map::new();
    for e in &present {
        let (a, b) = (&same[e], &other[e]);
        gate.insert(e.clone(), json!({
            "same_emotion_mean": r4(mean(a)),
            "best_other_emotion_mean": r4(mean(b)),
            "share_ge_gate": r4(a.iter().filter(|v| **v >= GATE).count() as f64 / a.len() as f64),
            "same_beats_other": r4(a.iter().zip(b).filter(|(x, y)| x > y).count() as f64 / a.len() as f64),
        }));
    }
    let report = json!({
        "schema": "galion-emotion12-bench/1",
        "corpus": name,
        "model": format!("emotion2vec+ {}", s.model),
        "base_input": if s.normalize { "funasr-layer-norm" } else { "raw" },
        "device": s.device,
        "emotions_config": o.emotions,
        "coverage": emo.coverage(labels).into_iter().collect::<BTreeMap<_, _>>(),
        "n": clips.len(),
        "seconds": r4(secs),
        "ms_per_clip": r4(1000.0 * secs / clips.len() as f64),
        "wa": r4(wa),
        "ua": r4(ua),
        "per_class_recall": recall.into_iter().map(|(k, v)| (k, r4(v))).collect::<BTreeMap<_, _>>(),
        "confusion": conf,
        "gate": GATE,
        "gate_formula": "0.70*category_blend+0.30*emb_sim (render = held-out actor clip, ref = same-emotion mean of the other actors)",
        "gate_metric": gate,
    });
    eprintln!("{name} {}: n={} WA={:.4} UA={:.4} {:.0}s on {}", s.model, clips.len(), wa, ua, secs, s.device);
    write(&o.out, &report)
}
