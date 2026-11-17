//! Per-emotion pass/fail matrix against the rollout gate.
//! Glue only: emotion posteriors/embeddings come from the ONNX scorer,
//! WER from galion-wer JSON, ECAPA from galion-ecapa JSON. The emotion score is
//! the locked harness formula `w_c*cos(mean posterior) + w_e*cos(mean embedding)`.
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Deserialize)]
pub struct Config {
    pub schema: String,
    pub gate: Gate,
    pub scorer: ScorerCfg,
    pub emotion: Vec<EmotionCfg>,
}
#[derive(Deserialize, Clone, Copy)]
pub struct Gate {
    pub wer_max: f64,
    pub ecapa_min: f64,
    pub emotion_min: f64,
    pub category_weight: f64,
    pub embedding_weight: f64,
}
#[derive(Deserialize)]
pub struct ScorerCfg {
    pub name: String,
    pub model: String,
    pub head: String,
}
#[derive(Deserialize)]
pub struct EmotionCfg {
    pub id: String,
    pub labels: Vec<String>,
}

pub fn load(path: &str) -> Result<Config> {
    let c: Config = toml::from_str(&std::fs::read_to_string(path).with_context(|| format!("read {path}"))?).with_context(|| format!("parse {path}"))?;
    if c.schema != "galion-emotion-eval/1" {
        bail!("{path}: schema {:?}, expected galion-emotion-eval/1", c.schema);
    }
    if c.emotion.is_empty() {
        bail!("{path}: no [[emotion]] rows");
    }
    if (c.gate.category_weight - 0.70).abs() > 1e-9 || (c.gate.embedding_weight - 0.30).abs() > 1e-9 || c.gate.emotion_min < 0.83 {
        bail!("{path}: the locked formula 0.70/0.30 and the 0.83 gate cannot be changed here");
    }
    Ok(c)
}

pub fn wavs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"))).map(|p| p.display().to_string()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub fn mean(vs: &[Vec<f32>]) -> Vec<f64> {
    let mut m = vec![0f64; vs[0].len()];
    for v in vs {
        for (a, b) in m.iter_mut().zip(v) {
            *a += *b as f64 / vs.len() as f64;
        }
    }
    m
}

pub fn cos(a: &[f64], b: &[f64]) -> f64 {
    let d: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { d / (na * nb) }
}

/// Read `summary.mean` (galion-wer) or `mean` (galion-ecapa) from a tool report.
pub fn tool_mean(path: &Path, pointer: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.pointer(pointer).and_then(Value::as_f64)
}

pub struct Row {
    pub id: String,
    pub n_render: usize,
    pub n_ref: usize,
    pub category_blend: Option<f64>,
    pub emb_sim: Option<f64>,
    pub target_prob: Option<f64>,
    pub wer: Option<f64>,
    pub ecapa: Option<f64>,
}

pub fn verdict(r: &Row, g: &Gate) -> (Option<f64>, &'static str, Vec<String>) {
    let em = match (r.category_blend, r.emb_sim) {
        (Some(c), Some(e)) => Some(g.category_weight * c + g.embedding_weight * e),
        _ => None,
    };
    let mut why = Vec::new();
    let mut missing = false;
    match em { Some(v) if v < g.emotion_min => why.push(format!("emotion {v:.3} < {}", g.emotion_min)), None => { missing = true; why.push("emotion unmeasured".into()) } _ => {} }
    match r.wer { Some(v) if v > g.wer_max => why.push(format!("wer {v:.3} > {}", g.wer_max)), None => { missing = true; why.push("wer unmeasured".into()) } _ => {} }
    match r.ecapa { Some(v) if v <= g.ecapa_min => why.push(format!("ecapa {v:.4} <= {}", g.ecapa_min)), None => { missing = true; why.push("ecapa unmeasured".into()) } _ => {} }
    let fails = why.iter().any(|w| !w.ends_with("unmeasured"));
    let status = if fails { "FAIL" } else if missing { "UNMEASURED" } else { "PASS" };
    (em, status, why)
}

pub fn render(cfg: &Config, rows: &[Row], device: &str, extra: Value) -> (Value, String) {
    let f = |x: Option<f64>, p: usize| x.map(|v| format!("{v:.p$}")).unwrap_or_else(|| "-".into());
    let mut md = format!(
        "# Emotion gate matrix ({} emotions)\n\nScorer: {} · device: {}\nGate: WER ≤ {} · ECAPA > {} · emotion_match ≥ {} (0.70·category + 0.30·embedding)\n\n| emotion | clips | refs | emotion_match | target prob | WER | ECAPA | result | why |\n|---|---:|---:|---:|---:|---:|---:|---|---|\n",
        cfg.emotion.len(), cfg.scorer.name, device, cfg.gate.wer_max, cfg.gate.ecapa_min, cfg.gate.emotion_min
    );
    let mut out = Vec::new();
    let (mut pass, mut fail, mut un) = (0, 0, 0);
    for r in rows {
        let (em, st, why) = verdict(r, &cfg.gate);
        match st { "PASS" => pass += 1, "FAIL" => fail += 1, _ => un += 1 }
        md += &format!("| {} | {} | {} | {} | {} | {} | {} | **{}** | {} |\n", r.id, r.n_render, r.n_ref, f(em, 3), f(r.target_prob, 3), f(r.wer, 3), f(r.ecapa, 4), st, if why.is_empty() { "-".into() } else { why.join("; ") });
        out.push(json!({"emotion": r.id, "clips": r.n_render, "refs": r.n_ref, "emotion_match": em, "category_blend": r.category_blend, "emb_sim": r.emb_sim,
            "target_prob": r.target_prob, "wer": r.wer, "ecapa": r.ecapa, "result": st, "why": why}));
    }
    md += &format!("\nTotal: {pass} PASS · {fail} FAIL · {un} UNMEASURED of {}. UNMEASURED is not a pass.\n", rows.len());
    let j = json!({"schema": "galion-emotion-matrix/1", "scorer": cfg.scorer.name, "device": device, "gate": {"wer_max": cfg.gate.wer_max, "ecapa_min": cfg.gate.ecapa_min, "emotion_min": cfg.gate.emotion_min},
        "totals": {"pass": pass, "fail": fail, "unmeasured": un, "emotions": rows.len()}, "rows": out, "run": extra});
    (j, md)
}

#[cfg(test)]
mod tests {
    use super::*;
    const G: Gate = Gate { wer_max: 0.018, ecapa_min: 0.7057, emotion_min: 0.83, category_weight: 0.7, embedding_weight: 0.3 };
    fn row(c: Option<f64>, w: Option<f64>, e: Option<f64>) -> Row {
        Row { id: "x".into(), n_render: 1, n_ref: 1, category_blend: c, emb_sim: c, target_prob: None, wer: w, ecapa: e }
    }
    #[test]
    fn gate_edges() {
        assert_eq!(verdict(&row(Some(0.83), Some(0.018), Some(0.7058)), &G).1, "PASS");
        assert_eq!(verdict(&row(Some(0.9), Some(0.018), Some(0.7057)), &G).1, "FAIL"); // ECAPA is strict
        assert_eq!(verdict(&row(Some(0.9), None, Some(0.8)), &G).1, "UNMEASURED");
        assert_eq!(verdict(&row(Some(0.5), None, Some(0.8)), &G).1, "FAIL");
    }
    #[test]
    fn locked_formula_enforced() {
        let t = std::env::temp_dir().join("galion-emotion-cfg-test.toml");
        std::fs::write(&t, "schema='galion-emotion-eval/1'\n[gate]\nwer_max=0.018\necapa_min=0.7057\nemotion_min=0.80\ncategory_weight=0.7\nembedding_weight=0.3\n[scorer]\nname='a'\nmodel='m'\nhead='h'\n[[emotion]]\nid='neutral'\nlabels=['neutral']\n").unwrap();
        assert!(load(t.to_str().unwrap()).is_err());
    }
    #[test]
    fn repo_config_parses() {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../emotion-v2/eval/emotions.toml");
        let c = load(p).unwrap();
        assert_eq!(c.emotion.len(), 14);
    }
}
