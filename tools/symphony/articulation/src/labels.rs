// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Reader for `configs/symphony/emotion-labels-decoder.toml`.
//! The cited direction tables stay in `emotion-labels-v2.toml` (a different schema).
//!
//! Values are z-scores against the speaker's own neutral, one column per
//! F0 band (`female` / `male`). Feature names are openSMILE eGeMAPSv02
//! functional names. A missing emotion or a missing neutral pass scores
//! nothing; it does not invent a z-score.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct FeatureScale {
    pub unit: String,
    pub neutral_std: f64,
    #[serde(default)]
    pub egemaps: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Labels {
    pub schema: u32,
    pub relative_to: String,
    /// Cowen 12 plus neutral. Empty when a file only carries z tables.
    #[serde(default)]
    pub set: Vec<String>,
    /// laughter, sigh, breath. Not members of `set`.
    #[serde(default)]
    pub bursts: Vec<String>,
    /// Render aliases. `seductive` = `desire`.
    #[serde(default)]
    pub alias: BTreeMap<String, String>,
    #[serde(default)]
    pub features: BTreeMap<String, FeatureScale>,
    #[serde(default)]
    pub emotions: BTreeMap<String, BTreeMap<String, BTreeMap<String, f64>>>,
}

pub fn parse(text: &str) -> Result<Labels, String> {
    let labels: Labels = toml::from_str(text).map_err(|e| e.to_string())?;
    if labels.schema != 2 {
        return Err(format!("emotion-labels schema {} (want 2)", labels.schema));
    }
    if labels.relative_to != "speaker_neutral" {
        return Err("emotion-labels relative_to must be speaker_neutral".into());
    }
    Ok(labels)
}

pub fn load(path: &Path) -> Result<Labels, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text)
}

/// z = (clip − neutral) / neutral_std for features the file scales in z units.
pub fn z_scores(
    clip: &BTreeMap<String, f64>,
    neutral: &BTreeMap<String, f64>,
    labels: &Labels,
) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for (name, scale) in &labels.features {
        if scale.unit != "z" || scale.neutral_std <= 0.0 {
            continue;
        }
        let (Some(c), Some(n)) = (clip.get(name), neutral.get(name)) else { continue };
        if c.is_finite() && n.is_finite() {
            out.insert(name.clone(), (c - n) / scale.neutral_std);
        }
    }
    out
}

/// Independent 0..1 match per emotion. Gaussian agreement with that column's target z.
pub fn match_emotions(z: &BTreeMap<String, f64>, labels: &Labels, column: &str) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for (emotion, cols) in &labels.emotions {
        let Some(targets) = cols.get(column) else { continue };
        let mut acc = 0.0;
        let mut n = 0;
        for (feat, target) in targets {
            let Some(have) = z.get(feat) else { continue };
            acc += (-0.5 * (have - target).powi(2)).exp();
            n += 1;
        }
        if n > 0 {
            out.insert(emotion.clone(), acc / n as f64);
        }
    }
    out
}
