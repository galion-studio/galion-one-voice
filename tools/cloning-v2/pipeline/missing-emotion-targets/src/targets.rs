//! Per-emotion reference-target file.
//! Each scorer category gets a numeric target and an origin:
//!   original-present  — detector strength on the strongest original window (≥ presence)
//!   synthesized       — detector strength on the best scored probe line
//! so a later gate can compare clone scores against these targets instead of skipping.

use crate::catalog::{alias_of, source_of};
use crate::detect::{strength_of, CategoryPresence, Detection, WindowScore};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ProbePick {
    pub category: String,
    pub best: Option<WindowScore>,
    pub best_strength: f64,
    pub all: Vec<(String, f64, String)>,
}

pub fn pick_best_probe(cat: &str, probes: &[WindowScore]) -> ProbePick {
    let mut all = Vec::new();
    let mut best: Option<(f64, &WindowScore)> = None;
    for p in probes {
        let s = strength_of(cat, p);
        all.push((p.id.clone(), s, p.path.clone()));
        if best.map(|(bs, _)| s > bs).unwrap_or(true) {
            best = Some((s, p));
        }
    }
    all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ProbePick {
        category: cat.to_string(),
        best_strength: best.map(|(s, _)| s).unwrap_or(0.0),
        best: best.map(|(_, p)| p.clone()),
        all,
    }
}

fn moment_snapshot(w: &WindowScore) -> Value {
    json!({
        "id": w.id,
        "path": w.path,
        "start_s": w.start_s,
        "end_s": w.end_s,
        "e2v_top": w.e2v_top,
        "clap_top": w.clap_top,
        "e2v_probs": w.e2v_probs,
        "clap_probs": w.clap_probs,
        "efficientat_max": w.eat_max,
    })
}

fn from_present(c: &CategoryPresence, windows: &[WindowScore]) -> Value {
    let window = c
        .best_window_id
        .as_ref()
        .and_then(|id| windows.iter().find(|w| &w.id == id));
    json!({
        "origin": "original-present",
        "target": c.strength,
        "strength": c.strength,
        "source": c.source,
        "detected": true,
        "window_id": c.best_window_id,
        "window_span": c.best_window_span,
        "moment_wav": c.best_window_path,
        "probe_score": Value::Null,
        "probe_scores": Value::Null,
        "moment": window.map(moment_snapshot),
    })
}

fn from_synthesized(c: &CategoryPresence, pick: Option<&ProbePick>, presence: f64) -> Value {
    let (target, detected, best, scores, moment) = if let Some(p) = pick {
        (
            json!(p.best_strength),
            json!(p.best_strength + 1e-12 >= presence),
            p.best.as_ref().map(|b| {
                json!({
                    "id": b.id,
                    "path": b.path,
                    "strength": p.best_strength,
                })
            }),
            json!(p.all.iter().map(|(id, s, path)| json!({
                "id": id, "strength": s, "path": path
            })).collect::<Vec<_>>()),
            p.best.as_ref().map(moment_snapshot),
        )
    } else {
        (Value::Null, json!(false), None, Value::Null, None)
    };
    json!({
        "origin": "synthesized",
        "target": target,
        "strength": c.strength,
        "original_max_below_presence": c.strength,
        "source": c.source,
        "detected": detected,
        "window_id": Value::Null,
        "window_span": Value::Null,
        "moment_wav": best.as_ref().and_then(|b| b.get("path").cloned()),
        "best_probe": best,
        "probe_score": target,
        "probe_scores": scores,
        "moment": moment,
    })
}

pub fn build_targets(
    voice: &str,
    original: &str,
    original_sha256: &str,
    presence: f64,
    detection: &Detection,
    windows: &[WindowScore],
    probes_by_cat: &BTreeMap<String, ProbePick>,
) -> Value {
    let mut emotions = serde_json::Map::new();
    for c in &detection.categories {
        let rec = if c.present {
            from_present(c, windows)
        } else {
            from_synthesized(c, probes_by_cat.get(&c.category), presence)
        };
        emotions.insert(c.category.clone(), rec);
    }
    // Alias rows point at the canonical emotion; they are not synthesized separately.
    for (alias, canon) in [("happy", "joyful"), ("reason", "logic")] {
        if let Some(src) = emotions.get(canon).cloned() {
            let mut obj = src;
            obj["alias_of"] = json!(canon);
            obj["source"] = json!(source_of(alias));
            emotions.insert(alias.to_string(), obj);
        }
    }
    let _ = alias_of; // documented in catalog
    json!({
        "schema": "galion-emotion-targets/1",
        "voice": voice,
        "original": original,
        "original_sha256": original_sha256,
        "presence_floor": presence,
        "formula": "For original-present emotions, target = detector strength on the best original window (same strength() as universal_match.py). For missing emotions (strength < presence), target = detector strength on the best synthesized probe line. origin marks synthesized vs original-present. The gate should compare clone category scores against these targets instead of skipping the category.",
        "backends": {
            "emotion2vec+": "trained class for joyful/aggressive/disgust/fear/sad/neutral/surprise",
            "efficientat": "PRIMARY for yelling/crying/laughter",
            "clap": "zero-shot for love/seductive/logic/reason"
        },
        "emotions": emotions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::detect;
    use std::collections::BTreeMap;

    fn w(id: &str, neu: f64, cry: f64) -> WindowScore {
        let mut e2v = BTreeMap::new();
        e2v.insert("neutral".into(), neu);
        e2v.insert("happy".into(), 0.0);
        e2v.insert("angry".into(), 0.0);
        e2v.insert("sad".into(), 0.0);
        let clap = BTreeMap::new();
        let mut eat = BTreeMap::new();
        eat.insert("crying".into(), cry);
        eat.insert("yelling".into(), 0.0);
        eat.insert("laughter".into(), 0.0);
        WindowScore {
            id: id.into(),
            path: format!("/tmp/{id}.wav"),
            start_s: 0.0,
            end_s: 2.5,
            e2v_probs: e2v,
            e2v_top: Some("neutral".into()),
            clap_probs: clap,
            clap_top: None,
            eat_max: eat,
            embedding: None,
        }
    }

    #[test]
    fn marks_synthesized_vs_original_present() {
        let windows = vec![w("w00", 1.0, 0.16)];
        let detection = detect(&windows, 0.2);
        let mut probe = w("cry01", 0.2, 0.55);
        probe.path = "/tmp/probes/cry01.wav".into();
        let pick = pick_best_probe("crying", &[probe]);
        let mut by = BTreeMap::new();
        by.insert("crying".into(), pick);
        let doc = build_targets("own-samantha", "/orig.wav", "abc", 0.2, &detection, &windows, &by);
        let neu = &doc["emotions"]["neutral"];
        assert_eq!(neu["origin"], "original-present");
        assert_eq!(neu["target"], 1.0);
        assert_eq!(neu["detected"], true);
        let cry = &doc["emotions"]["crying"];
        assert_eq!(cry["origin"], "synthesized");
        assert!((cry["target"].as_f64().unwrap() - 0.55).abs() < 1e-12);
        assert_eq!(cry["detected"], true);
        assert_eq!(cry["best_probe"]["id"], "cry01");
        assert_eq!(doc["emotions"]["happy"]["alias_of"], "joyful");
        assert_eq!(doc["emotions"]["reason"]["alias_of"], "logic");
    }

    #[test]
    fn pick_best_prefers_higher_strength() {
        let a = w("cry01", 0.1, 0.22);
        let b = w("cry02", 0.1, 0.41);
        let pick = pick_best_probe("crying", &[a, b]);
        assert_eq!(pick.best.as_ref().unwrap().id, "cry02");
        assert!((pick.best_strength - 0.41).abs() < 1e-12);
    }
}
