// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 Cowen categories, read from `configs/emotions-v2.toml`.
//! Neutral is the speaker baseline, not a twenty-fifth emotion.
//! Breath, sigh and laughter are rejected here on purpose.

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

    /// A planning state: one of the 24, or the neutral baseline.
    pub fn check_state(&self, name: &str) -> Result<(), String> {
        if name == "breath" || name == "sigh" || name == "laughter" {
            return Err(format!(
                "{name} is not an emotion. Breath is the physiology track. Sigh and laughter are vocal bursts."
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
}

pub fn bundled_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/emotions-v2.toml")
}
