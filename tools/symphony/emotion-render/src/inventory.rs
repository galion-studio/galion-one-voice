// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 names. The file `configs/emotions-v2.toml` is the inventory.
//! This enum matches that file; [`check_inventory_file`] fails if it drifts.
//!
//! Prosody is how a word is said. Burst is the same category with no words.
//! Neutral is the speaker baseline, not a twenty-fifth category.
//! Cowen, Elfenbein, Laukka & Keltner (2019), American Psychologist, 74(6),
//! Figure 3B. The twelve reliable-in-speech names are Cowen, Laukka,
//! Elfenbein, Liu & Keltner (2019), Nature Human Behaviour, 3(4).

use std::fs;
use std::path::Path;

use crate::error::RenderError;

pub const TICK_MS: u32 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Track {
    Prosody,
    Burst,
}

impl Track {
    pub fn as_str(self) -> &'static str {
        match self {
            Track::Prosody => "prosody",
            Track::Burst => "burst",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "prosody" => Some(Track::Prosody),
            "burst" => Some(Track::Burst),
            _ => None,
        }
    }
}

/// Confidence in prosody, not a gate. Both tiers are legal on both tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProsodyTier {
    /// Twelve dimensions the speech-prosody study preserved across the US and India.
    CrossCulturallyReliable,
    /// In the 24, not among those twelve. A word may still be said this way.
    ValidLowerRecognitionEvidence,
}

impl ProsodyTier {
    pub fn as_str(self) -> &'static str {
        match self {
            ProsodyTier::CrossCulturallyReliable => "cross-culturally reliable in prosody",
            ProsodyTier::ValidLowerRecognitionEvidence => {
                "valid in prosody, lower recognition evidence"
            }
        }
    }
}

/// Cowen 2019 names, inventory order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Adoration,
    Amusement,
    Anger,
    Awe,
    Confusion,
    Contempt,
    Contentment,
    Desire,
    Disappointment,
    Disgust,
    Distress,
    Ecstasy,
    Elation,
    Embarrassment,
    Fear,
    Interest,
    Pain,
    Realization,
    Relief,
    Sadness,
    SurpriseNegative,
    SurprisePositive,
    Sympathy,
    Triumph,
}

pub const CATEGORIES: [Category; 24] = [
    Category::Adoration,
    Category::Amusement,
    Category::Anger,
    Category::Awe,
    Category::Confusion,
    Category::Contempt,
    Category::Contentment,
    Category::Desire,
    Category::Disappointment,
    Category::Disgust,
    Category::Distress,
    Category::Ecstasy,
    Category::Elation,
    Category::Embarrassment,
    Category::Fear,
    Category::Interest,
    Category::Pain,
    Category::Realization,
    Category::Relief,
    Category::Sadness,
    Category::SurpriseNegative,
    Category::SurprisePositive,
    Category::Sympathy,
    Category::Triumph,
];

const RELIABLE: [Category; 12] = [
    Category::Adoration,
    Category::Amusement,
    Category::Anger,
    Category::Awe,
    Category::Confusion,
    Category::Contempt,
    Category::Desire,
    Category::Disappointment,
    Category::Distress,
    Category::Fear,
    Category::Interest,
    Category::Sadness,
];

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Adoration => "adoration",
            Category::Amusement => "amusement",
            Category::Anger => "anger",
            Category::Awe => "awe",
            Category::Confusion => "confusion",
            Category::Contempt => "contempt",
            Category::Contentment => "contentment",
            Category::Desire => "desire",
            Category::Disappointment => "disappointment",
            Category::Disgust => "disgust",
            Category::Distress => "distress",
            Category::Ecstasy => "ecstasy",
            Category::Elation => "elation",
            Category::Embarrassment => "embarrassment",
            Category::Fear => "fear",
            Category::Interest => "interest",
            Category::Pain => "pain",
            Category::Realization => "realization",
            Category::Relief => "relief",
            Category::Sadness => "sadness",
            Category::SurpriseNegative => "surprise_negative",
            Category::SurprisePositive => "surprise_positive",
            Category::Sympathy => "sympathy",
            Category::Triumph => "triumph",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        CATEGORIES.into_iter().find(|c| c.as_str() == s)
    }

    pub fn tier(self) -> ProsodyTier {
        if RELIABLE.contains(&self) {
            ProsodyTier::CrossCulturallyReliable
        } else {
            ProsodyTier::ValidLowerRecognitionEvidence
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Speaker baseline. z = 0. Not a category.
    Baseline,
    One {
        category: Category,
        /// Set when the old name is a burst channel (`laughter`).
        track: Option<Track>,
        alias_from: Option<String>,
    },
}

/// Old Galion names, plus the 24. A split is an error: the caller picks one.
pub fn resolve_name(name: &str) -> Result<Resolved, RenderError> {
    let key = name.trim().to_ascii_lowercase().replace('-', "_");
    let one = |category, track, alias: Option<&str>| Resolved::One {
        category,
        track,
        alias_from: alias.map(str::to_string),
    };
    match key.as_str() {
        "neutral" | "calm" => Ok(Resolved::Baseline),
        "tender" => Ok(one(Category::Adoration, None, Some("tender"))),
        "seductive" => Ok(one(Category::Desire, None, Some("seductive"))),
        "angry" => Ok(one(Category::Anger, None, Some("angry"))),
        "fearful" => Ok(one(Category::Fear, None, Some("fearful"))),
        "sad" => Ok(one(Category::Sadness, None, Some("sad"))),
        "disappointed" => Ok(one(Category::Disappointment, None, Some("disappointed"))),
        "amused" => Ok(one(Category::Amusement, None, Some("amused"))),
        // The old word maps to contempt. Disgust stays its own category.
        "disgusted" => Ok(one(Category::Contempt, None, Some("disgusted"))),
        "laughter" => Ok(one(
            Category::Amusement,
            Some(Track::Burst),
            Some("laughter"),
        )),
        "happy" => Err(RenderError::Split {
            name: key,
            options: vec!["amusement".into(), "interest".into()],
        }),
        "excited" => Err(RenderError::Split {
            name: key,
            options: vec!["interest".into(), "amusement".into()],
        }),
        "surprised" => Err(RenderError::Split {
            name: key,
            options: vec!["awe".into(), "confusion".into()],
        }),
        "sigh" => Err(RenderError::BurstDetector {
            name: key,
            may_be: vec![
                "relief".into(),
                "sadness".into(),
                "disappointment".into(),
                "contentment".into(),
            ],
        }),
        "guilt" | "pride" | "romantic_love" | "serenity" | "shame" => {
            Err(RenderError::NotACategory(key))
        }
        other => Category::parse(other)
            .map(|category| one(category, None, None))
            .ok_or_else(|| RenderError::NotACategory(other.to_string())),
    }
}

/// Fail if this crate's names drift from `configs/emotions-v2.toml`.
pub fn check_inventory_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let labels = string_list(
        v.get("inventory").and_then(|t| t.get("labels")),
        "inventory.labels",
    )?;
    let want: Vec<String> = CATEGORIES.iter().map(|c| c.as_str().to_string()).collect();
    if labels != want {
        return Err(format!(
            "inventory.labels drifted from the crate: {labels:?}"
        ));
    }
    let reliable = string_list(
        v.get("prosody_tier")
            .and_then(|t| t.get("cross_culturally_reliable"))
            .and_then(|t| t.get("labels")),
        "prosody tier reliable",
    )?;
    let lower = string_list(
        v.get("prosody_tier")
            .and_then(|t| t.get("valid_lower_recognition_evidence"))
            .and_then(|t| t.get("labels")),
        "prosody tier lower",
    )?;
    if reliable.len() != 12 || lower.len() != 12 {
        return Err("prosody tiers must be 12 and 12".into());
    }
    for name in &reliable {
        let cat = Category::parse(name).ok_or_else(|| format!("tier name {name}"))?;
        if cat.tier() != ProsodyTier::CrossCulturallyReliable {
            return Err(format!("{name} is not marked reliable in the crate"));
        }
    }
    for name in &lower {
        let cat = Category::parse(name).ok_or_else(|| format!("tier name {name}"))?;
        if cat.tier() != ProsodyTier::ValidLowerRecognitionEvidence {
            return Err(format!("{name} is not marked lower-tier in the crate"));
        }
    }
    if reliable.iter().any(|n| lower.contains(n)) {
        return Err("prosody tiers overlap".into());
    }
    let neutral = v
        .get("baseline")
        .and_then(|t| t.get("neutral"))
        .and_then(|t| t.get("not_emotion"))
        .and_then(|t| t.as_bool());
    if neutral != Some(true) {
        return Err("baseline.neutral.not_emotion must be true".into());
    }
    let breath = v
        .get("tracks")
        .and_then(|t| t.get("breath"))
        .and_then(|t| t.get("not_emotion"))
        .and_then(|t| t.as_bool());
    if breath != Some(true) {
        return Err("tracks.breath.not_emotion must be true".into());
    }
    Ok(())
}

fn string_list(v: Option<&toml::Value>, what: &str) -> Result<Vec<String>, String> {
    let arr = v
        .and_then(|x| x.as_array())
        .ok_or_else(|| format!("missing {what}"))?;
    arr.iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{what} entry is not a string"))
        })
        .collect()
}
