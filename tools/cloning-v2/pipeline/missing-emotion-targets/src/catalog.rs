//! Scorer category catalog and configurable probe texts/controls.
//!
//! Sources match `scorers/universal/py/universal_match.py`:
//!   emotion2vec+ trained classes, EfficientAT (yelling/crying/laughter),
//!   CLAP zero-shot (love/seductive/logic/reason).

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Blend IDs — same set as `universal_match.BLEND_IDS` (reason is an alias of logic).
pub const BLEND_IDS: &[&str] = &[
    "joyful",
    "aggressive",
    "disgust",
    "fear",
    "sad",
    "neutral",
    "surprise",
    "love",
    "seductive",
    "yelling",
    "crying",
    "laughter",
    "logic",
];

pub const EAT_STATES: &[&str] = &["yelling", "crying", "laughter"];

pub const CLAP_ZERO: &[&str] = &["love", "seductive", "logic", "reason"];

/// emotion2vec+ trained-class map (category -> e2v probability key).
pub fn e2v_trained_key(cat: &str) -> Option<&'static str> {
    match cat {
        "joyful" | "happy" => Some("happy"),
        "aggressive" => Some("angry"),
        "disgust" => Some("disgusted"),
        "fear" => Some("fearful"),
        "sad" => Some("sad"),
        "neutral" => Some("neutral"),
        "surprise" => Some("surprised"),
        _ => None,
    }
}

pub fn source_of(cat: &str) -> &'static str {
    if EAT_STATES.contains(&cat) {
        "efficientat"
    } else if CLAP_ZERO.contains(&cat) {
        "zero-shot text prompt (CLAP)"
    } else {
        "trained class (emotion2vec+)"
    }
}

pub fn alias_of(cat: &str) -> Option<&'static str> {
    match cat {
        "happy" => Some("joyful"),
        "reason" => Some("logic"),
        _ => None,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeLine {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeEmotion {
    pub pace: f64,
    #[serde(default)]
    pub notes: String,
    pub lines: Vec<ProbeLine>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeCatalog {
    pub version: u32,
    #[serde(default)]
    pub notes: String,
    pub emotions: BTreeMap<String, ProbeEmotion>,
}

impl ProbeCatalog {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read catalog {}", path.display()))?;
        let cat: Self = serde_json::from_str(&raw).context("parse probe catalog")?;
        cat.validate()?;
        Ok(cat)
    }

    pub fn validate(&self) -> Result<()> {
        for required in ["crying", "seductive"] {
            if !self.emotions.contains_key(required) {
                bail!("probe catalog missing required emotion {required}");
            }
        }
        for id in BLEND_IDS {
            if !self.emotions.contains_key(*id) {
                bail!("probe catalog missing scorer emotion {id}");
            }
        }
        for (id, emo) in &self.emotions {
            if emo.lines.is_empty() {
                bail!("probe catalog emotion {id} has no lines");
            }
            if !(emo.pace > 0.0) {
                bail!("probe catalog emotion {id} has non-positive pace");
            }
            for line in &emo.lines {
                if line.id.is_empty() || line.text.is_empty() {
                    bail!("probe catalog emotion {id} has an empty id or text");
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, cat: &str) -> Result<&ProbeEmotion> {
        self.emotions
            .get(cat)
            .with_context(|| format!("no probe lines configured for {cat}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn bundled() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("probe_catalog.json")
    }

    #[test]
    fn bundled_catalog_covers_scorer_emotions() {
        let cat = ProbeCatalog::load(&bundled()).unwrap();
        for id in BLEND_IDS {
            assert!(cat.emotions.contains_key(*id), "missing {id}");
            assert!(!cat.emotions[*id].lines.is_empty());
        }
        assert!(cat.emotions.contains_key("crying"));
        assert!(cat.emotions.contains_key("seductive"));
        assert!(cat.emotions["crying"].pace > 1.0);
        assert!(cat.emotions["seductive"].pace > 1.0);
    }

    #[test]
    fn source_labels_match_universal_scorer() {
        assert_eq!(source_of("crying"), "efficientat");
        assert_eq!(source_of("yelling"), "efficientat");
        assert_eq!(source_of("laughter"), "efficientat");
        assert_eq!(source_of("seductive"), "zero-shot text prompt (CLAP)");
        assert_eq!(source_of("love"), "zero-shot text prompt (CLAP)");
        assert_eq!(source_of("logic"), "zero-shot text prompt (CLAP)");
        assert_eq!(source_of("neutral"), "trained class (emotion2vec+)");
        assert_eq!(source_of("aggressive"), "trained class (emotion2vec+)");
        assert_eq!(e2v_trained_key("joyful"), Some("happy"));
        assert_eq!(e2v_trained_key("aggressive"), Some("angry"));
        assert_eq!(alias_of("happy"), Some("joyful"));
        assert_eq!(alias_of("reason"), Some("logic"));
    }
}
