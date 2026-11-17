//! Emotion list + emotion2vec+ class for each emotion, read from the Emotion V2
//! config (`emotion-v2/emotions.toml`, `[[emotions]] id / emotion2vec`). A
//! longer list (e.g. the 14-emotion file) works the same way. Nothing is
//! hard-coded here.
use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    id: String,
    emotion2vec: Option<String>,
    emotion2vec_in_set: Option<bool>,
}

#[derive(Deserialize)]
struct File {
    emotions: Vec<Row>,
}

pub struct Emotions {
    pub ids: Vec<String>,
    /// emotion2vec+ class name each emotion is read from (None = unreached).
    pub class: Vec<Option<String>>,
    pub in_set: Vec<bool>,
}

impl Emotions {
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
        let f: File = toml::from_str(&text).with_context(|| format!("parse {path}"))?;
        if f.emotions.is_empty() {
            bail!("{path}: no [[emotions]] rows");
        }
        let mut ids = Vec::new();
        let mut class = Vec::new();
        let mut in_set = Vec::new();
        for r in f.emotions {
            if ids.contains(&r.id) {
                bail!("{path}: duplicate emotion id {}", r.id);
            }
            in_set.push(r.emotion2vec_in_set.unwrap_or(false));
            class.push(r.emotion2vec.map(|c| c.trim().to_lowercase()));
            ids.push(r.id);
        }
        Ok(Self { ids, class, in_set })
    }

    /// Model posterior -> posterior over the configured emotions. A model class
    /// shared by several emotions is split evenly between them (the mapping in
    /// the config, nothing learned). Classes no emotion uses ("other",
    /// "unknown") are dropped and the rest renormalised.
    pub fn project(&self, model_labels: &[String], probs: &[f32]) -> Result<Vec<f64>> {
        let mut out = vec![0f64; self.ids.len()];
        let mut total = 0f64;
        for (label, p) in model_labels.iter().zip(probs) {
            let idx: Vec<usize> = (0..self.ids.len())
                .filter(|&i| self.class[i].as_deref() == Some(label.as_str()))
                .collect();
            if idx.is_empty() {
                continue;
            }
            for &i in &idx {
                out[i] += *p as f64 / idx.len() as f64;
            }
            total += *p as f64;
        }
        if total <= 0.0 {
            bail!("model posterior has no mass on any configured emotion");
        }
        Ok(out.into_iter().map(|v| v / total).collect())
    }

    /// Decision: most likely model class (among used ones), then the emotion
    /// marked in_set for it (else the first emotion that uses it).
    pub fn predict(&self, model_labels: &[String], probs: &[f32]) -> Option<String> {
        let mut best: Option<(&String, f32)> = None;
        for (label, p) in model_labels.iter().zip(probs) {
            if !self.class.iter().any(|c| c.as_deref() == Some(label.as_str())) {
                continue;
            }
            if best.map_or(true, |(_, b)| *p > b) {
                best = Some((label, *p));
            }
        }
        let (label, _) = best?;
        let users: Vec<usize> =
            (0..self.ids.len()).filter(|&i| self.class[i].as_deref() == Some(label.as_str())).collect();
        let pick = users.iter().find(|&&i| self.in_set[i]).or(users.first())?;
        Some(self.ids[*pick].clone())
    }

    pub fn coverage(&self, model_labels: &[String]) -> Vec<(String, String)> {
        (0..self.ids.len())
            .map(|i| {
                let kind = match &self.class[i] {
                    Some(c) if model_labels.contains(c) && self.in_set[i] => format!("native:{c}"),
                    Some(c) if model_labels.contains(c) => format!("shared:{c}"),
                    _ => "unreached".to_string(),
                };
                (self.ids[i].clone(), kind)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Emotions {
        Emotions::load(concat!(env!("CARGO_MANIFEST_DIR"), "/gate-emotions.toml")).unwrap()
    }

    fn labels() -> Vec<String> {
        crate::model::LABELS.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn gate_set_is_fourteen_and_all_reached() {
        let e = gate();
        assert_eq!(e.ids.len(), 14);
        assert_eq!(&e.ids[12..], ["laughter", "seductive"]);
        let cov = e.coverage(&labels());
        assert!(cov.iter().all(|(_, k)| k != "unreached"));
        assert_eq!(cov.iter().filter(|(_, k)| k.starts_with("native")).count(), 7);
    }

    #[test]
    fn projection_drops_other_and_splits_shared_classes() {
        let e = gate();
        // angry .1, happy .4, neutral .2, other .2, sad .1
        let p = [0.1, 0.0, 0.0, 0.4, 0.2, 0.2, 0.1, 0.0, 0.0];
        let out = e.project(&labels(), &p).unwrap();
        assert!((out.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let at = |id: &str| out[e.ids.iter().position(|x| x == id).unwrap()];
        assert!((at("angry") - 0.125).abs() < 1e-6);
        assert!((at("happy") - 0.5 / 6.0).abs() < 1e-6); // happy shared by 6 emotions
        assert!((at("seductive") - at("happy")).abs() < 1e-12);
        assert!((at("calm") - 0.125).abs() < 1e-6);
    }

    #[test]
    fn predict_uses_the_native_emotion_of_the_top_class() {
        let e = gate();
        let p = [0.0, 0.0, 0.0, 0.6, 0.4, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(e.predict(&labels(), &p).as_deref(), Some("happy"));
        let p = [0.0, 0.0, 0.0, 0.0, 0.0, 0.9, 0.1, 0.0, 0.0]; // "other" ignored
        assert_eq!(e.predict(&labels(), &p).as_deref(), Some("sad"));
    }

    #[test]
    fn twelve_emotion_v2_file_also_loads() {
        let e = Emotions::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../emotion-v2/emotions.toml")).unwrap();
        assert_eq!(e.ids.len(), 12);
    }
}
