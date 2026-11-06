//! Verified AudioSet class indices for EfficientAT mn10_as (527 classes).
//! Source: class_labels_indices.csv (qiuqiangkong/audioset_tagging_cnn metadata).

use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const N_CLASSES: usize = 527;
pub const EXPECTED_SHA256: &str = "bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504";

/// Shout(8), Yell(11), Screaming(14), Children shouting(13).
pub const YELLING_INDICES: &[usize] = &[8, 11, 14, 13];
/// Crying, sobbing (22) only — baby cry (23) excluded for adult clone eval.
pub const CRYING_INDICES: &[usize] = &[22];
/// Laughter(16), Baby laughter(17), Giggle(18), Belly laugh(20), Chuckle, chortle(21).
pub const LAUGHTER_INDICES: &[usize] = &[16, 17, 18, 20, 21];

#[allow(dead_code)]
pub const STATES: &[&str] = &["yelling", "crying", "laughter"];

#[derive(Clone, Debug, Default)]
pub struct StateScores {
    pub yelling: f32,
    pub crying: f32,
    pub laughter: f32,
    pub yelling_mean: f32,
    pub crying_mean: f32,
    pub laughter_mean: f32,
    pub per_class_max: BTreeMap<String, f32>,
}

impl StateScores {
    pub fn from_audioset(p: &[f32]) -> Self {
        assert!(p.len() >= N_CLASSES || p.len() == N_CLASSES);
        let take_max = |idxs: &[usize]| -> f32 {
            idxs.iter().map(|&i| p.get(i).copied().unwrap_or(0.0)).fold(0.0_f32, f32::max)
        };
        let mut per = BTreeMap::new();
        for &(name, idx) in &[
            ("Shout", 8usize), ("Yell", 11), ("Screaming", 14), ("Children shouting", 13),
            ("Crying, sobbing", 22),
            ("Laughter", 16), ("Baby laughter", 17), ("Giggle", 18), ("Belly laugh", 20), ("Chuckle, chortle", 21),
        ] {
            per.insert(name.to_string(), p.get(idx).copied().unwrap_or(0.0));
        }
        let y = take_max(YELLING_INDICES);
        let c = take_max(CRYING_INDICES);
        let l = take_max(LAUGHTER_INDICES);
        Self {
            yelling: y, crying: c, laughter: l,
            yelling_mean: y, crying_mean: c, laughter_mean: l,
            per_class_max: per,
        }
    }
}

pub fn state_map_json() -> Value {
    json!({
        "model": "Chris-Kode/efficientat-mn10-as-onnx",
        "sha256": EXPECTED_SHA256,
        "n_classes": N_CLASSES,
        "sample_rate": 32000,
        "states": {
            "yelling": {
                "source": "efficientat",
                "indices": {"Shout": 8, "Yell": 11, "Screaming": 14, "Children shouting": 13},
                "aggregate": "max over family, then max/mean over windows",
            },
            "crying": {
                "source": "efficientat",
                "indices": {"Crying, sobbing": 22},
                "aggregate": "max",
                "notes": "Baby cry, infant cry (23) excluded for adult Samantha/Jarvis clone eval",
            },
            "laughter": {
                "source": "efficientat",
                "indices": {"Laughter": 16, "Baby laughter": 17, "Giggle": 18, "Belly laugh": 20, "Chuckle, chortle": 21},
                "aggregate": "max over subtypes",
            }
        }
    })
}
