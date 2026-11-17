// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 Cowen 2019 names, copied from `configs/emotions-v2.toml`.
//!
//! TODO(S1): delete this list when `galion-mne-contract` is the inventory.
//! The crosswalk is the Galion map in ARTICULATION-V2-SPEC.md. It is not a
//! claim that the cited acoustic study used the new word.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    pub fn all() -> &'static [Category] {
        &ALL
    }

    /// Cowen, Laukka, Elfenbein, Liu and Keltner 2019, Nature Human Behaviour.
    /// A tier, not a gate. The other twelve are still legal on a word.
    pub fn prosody_tier(self) -> &'static str {
        match self {
            Category::Adoration
            | Category::Amusement
            | Category::Anger
            | Category::Awe
            | Category::Confusion
            | Category::Contempt
            | Category::Desire
            | Category::Disappointment
            | Category::Distress
            | Category::Fear
            | Category::Interest
            | Category::Sadness => "reliable",
            _ => "extended",
        }
    }
}

const ALL: [Category; 24] = [
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
}

pub fn parse_track(name: &str) -> Result<Track, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "prosody" | "speech" | "word" => Ok(Track::Prosody),
        "burst" => Ok(Track::Burst),
        other => Err(format!("unknown track {other}")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub category: Option<Category>,
    pub track_override: Option<Track>,
}

/// Resolve a name. Baseline names return `category: None`.
/// Ambiguous old names are an error, not a guess.
pub fn resolve(name: &str) -> Result<Resolved, String> {
    let key = name.trim().to_ascii_lowercase();
    let key = key.as_str();
    if matches!(key, "neutral" | "calm" | "baseline") {
        return Ok(Resolved {
            category: None,
            track_override: None,
        });
    }
    if key == "breath" || key == "inhale" || key == "exhale" {
        return Err("breath is physiology, not a category".into());
    }
    if let Some(cat) = parse_category(key) {
        return Ok(Resolved {
            category: Some(cat),
            track_override: None,
        });
    }
    let (cat, track) = match key {
        "tender" => (Category::Adoration, None),
        "seductive" => (Category::Desire, None),
        "angry" => (Category::Anger, None),
        "fearful" => (Category::Fear, None),
        "sad" => (Category::Sadness, None),
        "disappointed" => (Category::Disappointment, None),
        "amused" => (Category::Amusement, None),
        "disgusted" => (Category::Contempt, None),
        "laughter" | "laugh" => (Category::Amusement, Some(Track::Burst)),
        "happy" => {
            return Err(
                "happy is amusement and interest; it is not collapsed to one category".into(),
            )
        }
        "excited" => {
            return Err(
                "excited is interest and amusement; it is not collapsed to one category".into(),
            )
        }
        "surprised" => {
            return Err(
                "surprised is awe and confusion; it is not collapsed to one category".into(),
            )
        }
        other => return Err(format!("unknown category {other}")),
    };
    Ok(Resolved {
        category: Some(cat),
        track_override: track,
    })
}

fn parse_category(name: &str) -> Option<Category> {
    ALL.into_iter().find(|c| c.as_str() == name)
}
