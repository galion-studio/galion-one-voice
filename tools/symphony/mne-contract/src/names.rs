// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The 24 vocalization categories.
//!
//! Order matches `configs/emotions-v2.toml` `[inventory].labels`, which follows
//! Cowen, Elfenbein, Laukka and Keltner (2019), *American Psychologist*,
//! Figure 3B. The twelve Nature Human Behaviour names are a prosody tier,
//! not a shorter list. This file does not invent a twenty-fifth category.

use serde::{Deserialize, Serialize};

/// Figure 3B order. A test locks this to `configs/emotions-v2.toml`.
pub const COWEN: [&str; 24] = [
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "contentment",
    "desire",
    "disappointment",
    "disgust",
    "distress",
    "ecstasy",
    "elation",
    "embarrassment",
    "fear",
    "interest",
    "pain",
    "realization",
    "relief",
    "sadness",
    "surprise_negative",
    "surprise_positive",
    "sympathy",
    "triumph",
];

/// Cross-culturally reliable in prosody (Cowen et al. 2019, Nature Human Behaviour).
pub const RELIABLE: [&str; 12] = [
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "desire",
    "disappointment",
    "distress",
    "fear",
    "interest",
    "sadness",
];

/// Valid in prosody, lower recognition evidence. Still legal on a word.
pub const EXTENDED: [&str; 12] = [
    "contentment",
    "disgust",
    "ecstasy",
    "elation",
    "embarrassment",
    "pain",
    "realization",
    "relief",
    "surprise_negative",
    "surprise_positive",
    "sympathy",
    "triumph",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum CowenName {
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

impl CowenName {
    pub const ALL: [CowenName; 24] = [
        Self::Adoration,
        Self::Amusement,
        Self::Anger,
        Self::Awe,
        Self::Confusion,
        Self::Contempt,
        Self::Contentment,
        Self::Desire,
        Self::Disappointment,
        Self::Disgust,
        Self::Distress,
        Self::Ecstasy,
        Self::Elation,
        Self::Embarrassment,
        Self::Fear,
        Self::Interest,
        Self::Pain,
        Self::Realization,
        Self::Relief,
        Self::Sadness,
        Self::SurpriseNegative,
        Self::SurprisePositive,
        Self::Sympathy,
        Self::Triumph,
    ];

    pub fn as_str(self) -> &'static str {
        COWEN[self.index() as usize]
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }

    pub fn parse(name: &str) -> Option<Self> {
        let key = normalize(name);
        Self::ALL.into_iter().find(|id| id.as_str() == key)
    }

    pub fn tier(self) -> ProsodyTier {
        if RELIABLE.contains(&self.as_str()) {
            ProsodyTier::Reliable
        } else {
            ProsodyTier::Extended
        }
    }
}

/// Prosody confidence tier. Not a gate. Both tiers are legal on a spoken word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProsodyTier {
    Reliable,
    Extended,
}

impl ProsodyTier {
    /// Phrases from `configs/emotions-v2.toml`, not a new naming.
    pub fn phrase(self) -> &'static str {
        match self {
            Self::Reliable => "cross-culturally reliable in prosody",
            Self::Extended => "valid in prosody, lower recognition evidence",
        }
    }
}

/// Which track carries a pin. Baseline has no pin and no track byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    Prosody,
    Burst,
}

/// Result of [`resolve_name`]. The wire stores only a canonical pin or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedName {
    pub pin: Option<CowenName>,
    /// Meaningful when `pin` is `Some`. Burst is forced for `laughter`.
    pub track: Track,
    pub alias_from: Option<&'static str>,
}

pub fn normalize(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

/// Map a name onto the 24, or onto the speaker baseline.
///
/// Unambiguous old names, and only these, follow `configs/emotions-v2.toml`:
/// `calm` to baseline, `tender` to adoration, `seductive` to desire,
/// `laughter` to burst amusement, `disgusted` to contempt.
/// Splits (`happy`, `excited`, `surprised`, `sigh`) are refused.
/// `breath` is refused: it is physiology.
/// Names that did not form their own burst dimension are refused:
/// guilt, pride, romantic love, serenity, shame.
pub fn resolve_name(name: &str, track_if_canonical: Track) -> Result<ResolvedName, String> {
    let key = normalize(name);
    let pin = |name: CowenName, track: Track, alias: Option<&'static str>| ResolvedName {
        pin: Some(name),
        track,
        alias_from: alias,
    };
    let baseline = |alias: Option<&'static str>| ResolvedName {
        pin: None,
        track: Track::Prosody,
        alias_from: alias,
    };
    match key.as_str() {
        "neutral" | "baseline" => Ok(baseline(None)),
        "calm" => Ok(baseline(Some("calm"))),
        "tender" => Ok(pin(
            CowenName::Adoration,
            track_if_canonical,
            Some("tender"),
        )),
        "seductive" => Ok(pin(
            CowenName::Desire,
            track_if_canonical,
            Some("seductive"),
        )),
        "laughter" | "laugh" => Ok(pin(CowenName::Amusement, Track::Burst, Some("laughter"))),
        "disgusted" => Ok(pin(
            CowenName::Contempt,
            track_if_canonical,
            Some("disgusted"),
        )),
        "breath" => Err("breath is physiology, not a pin".into()),
        "sigh" => Err(
            "sigh splits across relief, sadness, disappointment, and contentment; name one".into(),
        ),
        "happy" => Err("happy splits into amusement and interest; name one".into()),
        "excited" => {
            Err("excited splits into interest and amusement; elation is its own category".into())
        }
        "surprised" => Err("surprised splits into awe and confusion; name one".into()),
        "guilt" | "pride" | "romantic_love" | "serenity" | "shame" => {
            Err(format!("{key} is not one of the 24"))
        }
        other => CowenName::parse(other)
            .map(|name| pin(name, track_if_canonical, None))
            .ok_or_else(|| format!("unknown emotion {name}")),
    }
}
