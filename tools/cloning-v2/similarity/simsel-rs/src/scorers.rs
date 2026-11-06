use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub const SCORERS_ROOT: &str = "/workspace/swarm/voice-v2-sprint/cloning-v2/scorers";

#[derive(Debug, Clone)]
pub struct ScorerBin {
    pub name: String,
    pub bin: PathBuf,
    pub ready_meta: Value,
}

#[derive(Debug, Deserialize)]
struct ReadyFile {
    #[serde(default)]
    bin: Option<String>,
    #[serde(default)]
    binary: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    command: Option<String>,
}

fn candidate_bins(name: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(SCORERS_ROOT).join(name);
    let mut v = vec![
        root.join("target/release").join(format!("galion-{name}")),
        root.join("target/release").join(name),
        root.join("target/release").join(format!("galion_{name}")),
        root.join(format!("galion-{name}")),
        root.join("bin").join(format!("galion-{name}")),
        root.join("bin").join(name),
    ];
    if name == "articulation" {
        v.push(root.join("galion-artic"));
        v.push(root.join("target/release").join("galion-artic"));
    }
    v
}

pub fn ready_path(name: &str) -> PathBuf {
    PathBuf::from(SCORERS_ROOT).join(name).join("READY")
}

pub fn is_ready(name: &str) -> bool {
    if ready_path(name).is_file() {
        return true;
    }
    // Provisional: accept a built release binary before the worker writes READY.
    candidate_bins(name).into_iter().any(|p| p.is_file())
}

pub fn load_scorer(name: &str) -> Result<ScorerBin> {
    let rp = ready_path(name);
    let raw = if rp.is_file() {
        fs::read_to_string(&rp).unwrap_or_default()
    } else if candidate_bins(name).into_iter().any(|p| p.is_file()) {
        String::new()
    } else {
        bail!("scorer '{name}' not ready (missing {} and no release binary)", rp.display());
    };
    let trimmed = raw.trim().to_string();
    let meta: Value = serde_json::from_str(&trimmed).unwrap_or_else(|_| json!({"raw": trimmed}));
    let parsed: ReadyFile = serde_json::from_str(&trimmed).unwrap_or(ReadyFile {
        bin: None,
        binary: None,
        path: None,
        command: None,
    });
    let mut bins: Vec<PathBuf> = [parsed.bin, parsed.binary, parsed.path, parsed.command]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .collect();
    // plain-text READY containing a filesystem path
    if bins.is_empty() && trimmed.starts_with('/') && !trimmed.contains('\n') {
        bins.push(PathBuf::from(&trimmed));
    }
    bins.extend(candidate_bins(name));
    let bin = bins
        .into_iter()
        .find(|p| p.is_file())
        .with_context(|| format!("READY present for {name} but no binary found"))?;
    Ok(ScorerBin {
        name: name.to_string(),
        bin,
        ready_meta: meta,
    })
}

pub fn wait_ready(names: &[&str], timeout_s: u64) -> Result<Vec<ScorerBin>> {
    let t0 = Instant::now();
    loop {
        let mut ok = true;
        for n in names {
            if !is_ready(n) {
                ok = false;
                break;
            }
        }
        if ok {
            let mut out = Vec::new();
            for n in names {
                out.push(load_scorer(n)?);
            }
            return Ok(out);
        }
        if t0.elapsed() > Duration::from_secs(timeout_s) {
            let missing: Vec<_> = names.iter().filter(|n| !is_ready(n)).collect();
            bail!("timeout waiting for scorers: {:?}", missing);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Run a scorer CLI. Convention (flexible):
///   galion-ecapa --hyp H --ref R [--json]
///   galion-emotion --hyp H --ref R [--json]  -> distance / embedding distance
/// Falls back to: bin H R
pub fn run_pair_scorer(scorer: &ScorerBin, hyp: &Path, reference: &Path) -> Result<(Value, f64)> {
    let t0 = Instant::now();
    let hyp_s = hyp.display().to_string();
    let ref_s = reference.display().to_string();
    // Prefer scorer-native CLIs:
    //   galion-emotion score --ref REF --threads 2 HYP
    //   galion-ecapa --ref REF HYP [--threads 2]
    let attempts: Vec<Vec<String>> = match scorer.name.as_str() {
        "emotion" => vec![
            vec![
                "score".into(),
                "--ref".into(),
                ref_s.clone(),
                "--threads".into(),
                "2".into(),
                "--compact".into(),
                hyp_s.clone(),
            ],
            vec![
                "score".into(),
                "--ref".into(),
                ref_s.clone(),
                "--threads".into(),
                "2".into(),
                hyp_s.clone(),
            ],
        ],
        "ecapa" => vec![
            vec![
                "--ref".into(),
                ref_s.clone(),
                hyp_s.clone(),
                "--threads".into(),
                "2".into(),
            ],
            vec!["--ref".into(), ref_s.clone(), hyp_s.clone()],
        ],
        _ => vec![
            vec![
                "--hyp".into(),
                hyp_s.clone(),
                "--ref".into(),
                ref_s.clone(),
                "--json".into(),
            ],
            vec![hyp_s.clone(), ref_s.clone()],
        ],
    };
    let mut last_err = None;
    for args in &attempts {
        let out = Command::new(&scorer.bin).args(args).output();
        match out {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let val: Value =
                    serde_json::from_str(&stdout).unwrap_or_else(|_| json!({"raw": stdout}));
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                return Ok((val, ms));
            }
            Ok(o) => {
                last_err = Some(format!(
                    "status={} stderr={}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr)
                ));
            }
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    bail!(
        "scorer {} failed: {}",
        scorer.bin.display(),
        last_err.unwrap_or_default()
    );
}

pub fn extract_ecapa_sim(v: &Value) -> Option<f64> {
    if let Some(m) = v.get("mean").and_then(|x| x.as_f64()) {
        return Some(m);
    }
    if let Some(scores) = v.get("scores").and_then(|x| x.as_object()) {
        // first value
        for (_k, val) in scores {
            if let Some(x) = val.as_f64() {
                return Some(x);
            }
        }
    }
    for key in ["similarity", "cosine", "ecapa", "score", "sim"] {
        if let Some(x) = v.get(key).and_then(|x| x.as_f64()) {
            return Some(x);
        }
    }
    None
}

/// Emotion distance to reference (lower is better). Prefer galion-emotion fields.
pub fn extract_emotion_distance(v: &Value) -> Option<f64> {
    if let Some(x) = v
        .pointer("/summary/mean_emotion_distance")
        .and_then(|x| x.as_f64())
    {
        return Some(x);
    }
    // files[0].vs_ref.emotion_distance
    if let Some(files) = v.get("files").and_then(|x| x.as_array()) {
        if let Some(f0) = files.first() {
            if let Some(x) = f0
                .pointer("/vs_ref/emotion_distance")
                .and_then(|x| x.as_f64())
            {
                return Some(x);
            }
        }
    }
    // single-file object
    if let Some(x) = v.pointer("/vs_ref/emotion_distance").and_then(|x| x.as_f64()) {
        return Some(x);
    }
    for key in [
        "emotion_distance",
        "distance",
        "galion_emotion_distance",
        "dist",
        "l2",
        "cosine_distance",
    ] {
        if let Some(x) = v.get(key).and_then(|x| x.as_f64()) {
            return Some(x);
        }
    }
    if let Some(s) = v.get("emotion_similarity").and_then(|x| x.as_f64()) {
        return Some(1.0 - s);
    }
    None
}

pub fn scorers_status() -> Value {
    let names = ["ecapa", "articulation", "quality", "emotion"];
    let mut map = serde_json::Map::new();
    for n in names {
        let ready = is_ready(n);
        let mut entry = json!({"ready": ready, "ready_path": ready_path(n).display().to_string()});
        if ready {
            if let Ok(s) = load_scorer(n) {
                entry["bin"] = json!(s.bin.display().to_string());
            }
        }
        map.insert(n.to_string(), entry);
    }
    Value::Object(map)
}
