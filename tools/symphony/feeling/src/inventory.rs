// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 names, read from `configs/emotions-v2.toml`.
//! They are optional pins. The feeling state is not this list.
//! Neutral is the baseline. Breath is not a name here.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Track {
    Prosody,
    Burst,
}

impl Track {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "prosody" => Ok(Track::Prosody),
            "burst" => Ok(Track::Burst),
            _ => Err(format!("track must be prosody or burst (got {s})")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Track::Prosody => "prosody",
            Track::Burst => "burst",
        }
    }
}

#[derive(Clone, Debug)]
struct MapRow {
    to: Vec<String>,
    to_track: String,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    labels: Vec<String>,
    maps: BTreeMap<String, MapRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    Baseline {
        alias_from: String,
    },
    One {
        category: String,
        track: Option<Track>,
        alias_from: Option<String>,
    },
    Split {
        alias_from: String,
        to: Vec<String>,
        track: Option<Track>,
    },
    /// A name that is not one of the 24 and not an old alias.
    /// It is not a pin. It does not move the space.
    Unknown {
        id: String,
    },
}

impl Inventory {
    pub fn bundled_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/emotions-v2.toml")
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let value: toml::Value =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let inventory = value
            .get("inventory")
            .ok_or_else(|| format!("{}: missing [inventory]", path.display()))?;
        let arr = inventory
            .get("labels")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("{}: inventory.labels", path.display()))?;
        let labels: Vec<String> = arr
            .iter()
            .map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("{}: a label is not a string", path.display()))?;
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
        if let Some(n) = inventory.get("n").and_then(|v| v.as_integer()) {
            if n as usize != labels.len() {
                return Err(format!(
                    "{}: inventory.n is {n} but {} labels are listed",
                    path.display(),
                    labels.len()
                ));
            }
        }
        let mut maps = BTreeMap::new();
        if let Some(table) = value.get("map").and_then(|v| v.as_table()) {
            for (name, body) in table {
                let to = body
                    .get("to")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| format!("{}: map.{name}.to", path.display()))?;
                let to: Vec<String> = to
                    .iter()
                    .map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| format!("{}: map.{name}.to", path.display()))?;
                if to.is_empty() {
                    return Err(format!("{}: map.{name}.to is empty", path.display()));
                }
                let to_track = body
                    .get("to_track")
                    .and_then(|v| v.as_str())
                    .unwrap_or("prosody")
                    .to_string();
                for dest in &to {
                    if dest != "neutral" && !labels.iter().any(|s| s == dest) {
                        return Err(format!(
                            "{}: map.{name} points at {dest}, which is not in the inventory",
                            path.display()
                        ));
                    }
                }
                maps.insert(name.clone(), MapRow { to, to_track });
            }
        }
        Ok(Inventory { labels, maps })
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    pub fn is_category(&self, name: &str) -> bool {
        self.labels.iter().any(|s| s == name)
    }

    pub fn resolve(&self, id: &str) -> Result<Resolved, String> {
        if id == "breath" {
            return Err("breath is not a feeling. Breath is physiology.".into());
        }
        if self.is_category(id) {
            return Ok(Resolved::One {
                category: id.to_string(),
                track: None,
                alias_from: None,
            });
        }
        if id == "neutral" {
            return Ok(Resolved::Baseline {
                alias_from: id.to_string(),
            });
        }
        if let Some(row) = self.maps.get(id) {
            let track = match row.to_track.as_str() {
                "baseline" => None,
                other => Some(Track::parse(other)?),
            };
            if row.to_track == "baseline" {
                return Ok(Resolved::Baseline {
                    alias_from: id.to_string(),
                });
            }
            if row.to.len() == 1 {
                return Ok(Resolved::One {
                    category: row.to[0].clone(),
                    track,
                    alias_from: Some(id.to_string()),
                });
            }
            return Ok(Resolved::Split {
                alias_from: id.to_string(),
                to: row.to.clone(),
                track,
            });
        }
        Ok(Resolved::Unknown { id: id.to_string() })
    }
}
