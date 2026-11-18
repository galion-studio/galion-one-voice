// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 Cowen categories, read from `configs/emotions-v2.toml`.
//! Neutral is the speaker baseline, not a twenty-fifth emotion.
//! Breath, sigh and laughter are not categories. They are a physiology track
//! or a wordless gesture.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Inventory {
    pub labels: Vec<String>,
}

impl Inventory {
    pub fn bundled() -> Result<Self, String> {
        Self::load(&bundled_path())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let value: toml::Value =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let inventory = value
            .get("inventory")
            .ok_or_else(|| format!("{}: missing [inventory]", path.display()))?;
        let arr = inventory
            .get("labels")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("{}: inventory.labels must be an array", path.display()))?;
        let labels: Vec<String> = arr
            .iter()
            .map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("{}: a label is not a string", path.display()))?;
        if let Some(n) = inventory.get("n").and_then(|v| v.as_integer()) {
            if n as usize != labels.len() {
                return Err(format!(
                    "{}: inventory.n is {n} but {} labels are listed",
                    path.display(),
                    labels.len()
                ));
            }
        }
        if labels.iter().any(|s| s == "breath" || s == "neutral") {
            return Err(format!(
                "{}: breath and neutral are not inventory labels",
                path.display()
            ));
        }
        let mut seen = labels.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != labels.len() {
            return Err(format!("{}: duplicate emotion label", path.display()));
        }
        Ok(Inventory { labels })
    }

    pub fn is_category(&self, name: &str) -> bool {
        self.labels.iter().any(|s| s == name)
    }

    /// Prosody state: one of the 24, or the neutral baseline.
    pub fn check_prosody(&self, name: &str) -> Result<(), String> {
        if name == "breath" {
            return Err("breath is physiology, not an emotion".into());
        }
        if name == "sigh" || name == "laughter" || name == "laugh" {
            return Err(format!(
                "{name} is a wordless gesture, not a prosody category. A laugh is burst amusement. A sigh is not one category."
            ));
        }
        if name == "neutral" || self.is_category(name) {
            return Ok(());
        }
        Err(format!(
            "{name} is not one of the {} Cowen categories and is not the neutral baseline",
            self.labels.len()
        ))
    }

    pub fn check_burst_category(&self, name: &str) -> Result<(), String> {
        if name == "neutral" {
            return Err("the burst track has no neutral. Neutral is the speaker baseline.".into());
        }
        if self.is_category(name) {
            return Ok(());
        }
        Err(format!(
            "{name} is not one of the {} Cowen categories",
            self.labels.len()
        ))
    }
}

pub fn bundled_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/emotions-v2.toml")
}
