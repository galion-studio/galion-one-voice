//! Detect which universal-scorer categories fall below the presence floor.
//! Strength formula copied from `scorers/universal/py/universal_match.py:strength`
//! (invoked against JSON produced by the unmodified scorer).

use crate::catalog::{e2v_trained_key, source_of, BLEND_IDS, EAT_STATES};
use crate::windows::WindowMeta;
use crate::PRESENCE;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowScore {
    pub id: String,
    pub path: String,
    pub start_s: f64,
    pub end_s: f64,
    pub e2v_probs: BTreeMap<String, f64>,
    pub e2v_top: Option<String>,
    pub clap_probs: BTreeMap<String, f64>,
    pub clap_top: Option<String>,
    pub eat_max: BTreeMap<String, f64>,
    pub embedding: Option<Vec<f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryPresence {
    pub category: String,
    pub source: String,
    pub strength: f64,
    pub present: bool,
    pub best_window_id: Option<String>,
    pub best_window_span: Option<String>,
    pub best_window_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Detection {
    pub presence_floor: f64,
    pub categories: Vec<CategoryPresence>,
}

impl Detection {
    pub fn missing(&self) -> Vec<&CategoryPresence> {
        self.categories.iter().filter(|c| !c.present).collect()
    }
    pub fn present(&self) -> Vec<&CategoryPresence> {
        self.categories.iter().filter(|c| c.present).collect()
    }
}

/// Same as Python `strength(cat, e2v_probs, clap_probs, eat_states)`.
pub fn strength(cat: &str, e2v_probs: &BTreeMap<String, f64>, clap_probs: &BTreeMap<String, f64>, eat_max: &BTreeMap<String, f64>) -> f64 {
    if EAT_STATES.contains(&cat) {
        return eat_max.get(cat).copied().unwrap_or(0.0);
    }
    if let Some(key) = e2v_trained_key(cat) {
        return e2v_probs.get(key).copied().unwrap_or(0.0);
    }
    clap_probs.get(cat).copied().unwrap_or(0.0)
}

pub fn strength_of(cat: &str, w: &WindowScore) -> f64 {
    strength(cat, &w.e2v_probs, &w.clap_probs, &w.eat_max)
}

fn f64_field(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn map_f64(v: Option<&Value>) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if let Some(Value::Object(map)) = v {
        for (k, val) in map {
            if let Some(n) = val.as_f64() {
                out.insert(k.clone(), n);
            }
        }
    }
    out
}

fn eat_max_map(eat: Option<&Value>) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let Some(Value::Object(map)) = eat else {
        return out;
    };
    for (k, val) in map {
        let n = if let Some(x) = val.as_f64() {
            x
        } else if val.is_object() {
            f64_field(val, "max")
        } else {
            0.0
        };
        out.insert(k.clone(), n);
    }
    out
}

pub fn parse_score_file(doc: &Value, windows: &[WindowMeta]) -> Result<Vec<WindowScore>> {
    let files = doc
        .get("files")
        .and_then(|v| v.as_array())
        .context("score JSON missing files[]")?;
    let mut by_name: BTreeMap<String, &Value> = BTreeMap::new();
    let mut by_path: BTreeMap<String, &Value> = BTreeMap::new();
    for f in files {
        let path = f.get("file").and_then(|x| x.as_str()).unwrap_or("");
        by_path.insert(path.to_string(), f);
        if let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) {
            by_name.insert(name.to_string(), f);
        }
    }
    let mut out = Vec::new();
    for meta in windows {
        let rec = by_path
            .get(&meta.path)
            .or_else(|| {
                Path::new(&meta.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| by_name.get(n))
            })
            .with_context(|| format!("scorer output missing {}", meta.path))?;
        let e2v = rec.get("e2v").unwrap_or(&Value::Null);
        let clap = rec.get("clap").unwrap_or(&Value::Null);
        let embedding = e2v
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect());
        out.push(WindowScore {
            id: meta.id.clone(),
            path: meta.path.clone(),
            start_s: meta.start_s,
            end_s: meta.end_s,
            e2v_probs: map_f64(e2v.get("probs")),
            e2v_top: e2v.get("top").and_then(|x| x.as_str()).map(str::to_string),
            clap_probs: map_f64(clap.get("probs")),
            clap_top: clap.get("top").and_then(|x| x.as_str()).map(str::to_string),
            eat_max: eat_max_map(rec.get("efficientat")),
            embedding,
        });
    }
    Ok(out)
}

/// Also parse probe wav scores (one record per file, id from stem).
pub fn parse_probe_scores(doc: &Value) -> Result<Vec<WindowScore>> {
    let files = doc
        .get("files")
        .and_then(|v| v.as_array())
        .context("probe score JSON missing files[]")?;
    let mut out = Vec::new();
    for rec in files {
        let path = rec
            .get("file")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let id = Path::new(&path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("probe")
            .to_string();
        let e2v = rec.get("e2v").unwrap_or(&Value::Null);
        let clap = rec.get("clap").unwrap_or(&Value::Null);
        let embedding = e2v
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect());
        out.push(WindowScore {
            id,
            path,
            start_s: 0.0,
            end_s: rec.get("duration_s").and_then(|x| x.as_f64()).unwrap_or(0.0),
            e2v_probs: map_f64(e2v.get("probs")),
            e2v_top: e2v.get("top").and_then(|x| x.as_str()).map(str::to_string),
            clap_probs: map_f64(clap.get("probs")),
            clap_top: clap.get("top").and_then(|x| x.as_str()).map(str::to_string),
            eat_max: eat_max_map(rec.get("efficientat")),
            embedding,
        });
    }
    Ok(out)
}

pub fn detect(windows: &[WindowScore], presence: f64) -> Detection {
    let mut categories = Vec::new();
    for cat in BLEND_IDS {
        let mut best: Option<(f64, &WindowScore)> = None;
        for w in windows {
            let s = strength_of(cat, w);
            if s + 1e-12 >= presence && best.map(|(bs, _)| s > bs).unwrap_or(true) {
                best = Some((s, w));
            }
        }
        if let Some((s, w)) = best {
            categories.push(CategoryPresence {
                category: (*cat).to_string(),
                source: source_of(cat).to_string(),
                strength: s,
                present: true,
                best_window_id: Some(w.id.clone()),
                best_window_span: Some(format!("{:.3}-{:.3}", w.start_s, w.end_s)),
                best_window_path: Some(w.path.clone()),
            });
        } else {
            let max_s = windows
                .iter()
                .map(|w| strength_of(cat, w))
                .fold(0.0_f64, f64::max);
            categories.push(CategoryPresence {
                category: (*cat).to_string(),
                source: source_of(cat).to_string(),
                strength: max_s,
                present: false,
                best_window_id: None,
                best_window_span: None,
                best_window_path: None,
            });
        }
    }
    Detection {
        presence_floor: presence,
        categories,
    }
}

pub fn default_presence() -> f64 {
    PRESENCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(
        id: &str,
        start: f64,
        end: f64,
        neu: f64,
        hap: f64,
        ang: f64,
        sad: f64,
        love: f64,
        sed: f64,
        logic: f64,
        yell: f64,
        cry: f64,
        laugh: f64,
    ) -> WindowScore {
        let mut e2v = BTreeMap::new();
        e2v.insert("neutral".into(), neu);
        e2v.insert("happy".into(), hap);
        e2v.insert("angry".into(), ang);
        e2v.insert("sad".into(), sad);
        e2v.insert("disgusted".into(), 0.01);
        e2v.insert("fearful".into(), 0.01);
        e2v.insert("surprised".into(), 0.0);
        let mut clap = BTreeMap::new();
        clap.insert("love".into(), love);
        clap.insert("seductive".into(), sed);
        clap.insert("logic".into(), logic);
        clap.insert("reason".into(), logic * 0.5);
        let mut eat = BTreeMap::new();
        eat.insert("yelling".into(), yell);
        eat.insert("crying".into(), cry);
        eat.insert("laughter".into(), laugh);
        WindowScore {
            id: id.into(),
            path: format!("{id}.wav"),
            start_s: start,
            end_s: end,
            e2v_probs: e2v,
            e2v_top: Some("neutral".into()),
            clap_probs: clap,
            clap_top: Some("logic".into()),
            eat_max: eat,
            embedding: None,
        }
    }

    #[test]
    fn presence_boundary_is_inclusive() {
        let w = rec("w00", 0.0, 2.5, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.1, 0.0, 0.2, 0.0);
        assert!((strength_of("crying", &w) - 0.2).abs() < 1e-15);
        let d = detect(&[w], 0.2);
        let cry = d.categories.iter().find(|c| c.category == "crying").unwrap();
        assert!(cry.present, "0.2 should count as present");
        let sed = d.categories.iter().find(|c| c.category == "seductive").unwrap();
        assert!(!sed.present);
    }

    #[test]
    fn below_presence_is_missing() {
        let w = rec("w00", 0.0, 2.5, 1.0, 0.12, 0.0, 0.07, 0.1, 0.102, 0.5, 0.66, 0.161, 0.27);
        let d = detect(&[w], 0.2);
        let missing: Vec<_> = d.missing().into_iter().map(|c| c.category.as_str()).collect();
        assert!(missing.contains(&"crying"));
        assert!(missing.contains(&"seductive"));
        assert!(missing.contains(&"joyful"));
        let present: Vec<_> = d.present().into_iter().map(|c| c.category.as_str()).collect();
        assert!(present.contains(&"neutral"));
        assert!(present.contains(&"logic"));
        assert!(present.contains(&"yelling"));
        assert!(present.contains(&"laughter"));
    }

    #[test]
    fn parse_score_universal_eat_object() {
        let doc = json!({
            "files": [{
                "file": "/tmp/w00_0.000_2.500.wav",
                "e2v": {"probs": {"neutral": 1.0, "happy": 0.0, "angry": 0.0, "sad": 0.0, "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0}, "top": "neutral"},
                "clap": {"probs": {"love": 0.1, "seductive": 0.05, "logic": 0.6}, "top": "logic"},
                "efficientat": {
                    "crying": {"max": 0.161, "mean": 0.161, "source": "efficientat"},
                    "laughter": {"max": 0.274, "mean": 0.274},
                    "yelling": {"max": 0.442}
                }
            }]
        });
        let meta = [WindowMeta {
            id: "w00".into(),
            path: "/tmp/w00_0.000_2.500.wav".into(),
            start_s: 0.0,
            end_s: 2.5,
        }];
        let scored = parse_score_file(&doc, &meta).unwrap();
        assert!((scored[0].eat_max["crying"] - 0.161).abs() < 1e-12);
        assert!((strength_of("laughter", &scored[0]) - 0.274).abs() < 1e-12);
        assert!((strength_of("neutral", &scored[0]) - 1.0).abs() < 1e-12);
        assert!((strength_of("logic", &scored[0]) - 0.6).abs() < 1e-12);
    }
}
