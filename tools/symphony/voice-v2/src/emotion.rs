// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One shared set of 24 categories for the prosody track and the burst track.
//!
//! The 12 Nature Human Behaviour names are the ones that paper measured as
//! reliable in prosody. They are a confidence tier. They are not the only
//! feelings a spoken word is allowed to carry.

use serde::{Deserialize, Serialize};

use crate::fluid::Avd;

/// Categories, in tier order: the 12 reliable-in-prosody names, then the other 12.
// `Ord` is declaration order so a map can count ticks. It is not a ranking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmotionId {
    Adoration,
    Amusement,
    Anger,
    Awe,
    Confusion,
    Contempt,
    Desire,
    Disappointment,
    Distress,
    Fear,
    Interest,
    Sadness,
    Contentment,
    Disgust,
    Ecstasy,
    Elation,
    Embarrassment,
    Pain,
    Realization,
    Relief,
    SurpriseNegative,
    SurprisePositive,
    Sympathy,
    Triumph,
}

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

impl EmotionId {
    pub const ALL: [EmotionId; 24] = [
        Self::Adoration,
        Self::Amusement,
        Self::Anger,
        Self::Awe,
        Self::Confusion,
        Self::Contempt,
        Self::Desire,
        Self::Disappointment,
        Self::Distress,
        Self::Fear,
        Self::Interest,
        Self::Sadness,
        Self::Contentment,
        Self::Disgust,
        Self::Ecstasy,
        Self::Elation,
        Self::Embarrassment,
        Self::Pain,
        Self::Realization,
        Self::Relief,
        Self::SurpriseNegative,
        Self::SurprisePositive,
        Self::Sympathy,
        Self::Triumph,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adoration => "adoration",
            Self::Amusement => "amusement",
            Self::Anger => "anger",
            Self::Awe => "awe",
            Self::Confusion => "confusion",
            Self::Contempt => "contempt",
            Self::Desire => "desire",
            Self::Disappointment => "disappointment",
            Self::Distress => "distress",
            Self::Fear => "fear",
            Self::Interest => "interest",
            Self::Sadness => "sadness",
            Self::Contentment => "contentment",
            Self::Disgust => "disgust",
            Self::Ecstasy => "ecstasy",
            Self::Elation => "elation",
            Self::Embarrassment => "embarrassment",
            Self::Pain => "pain",
            Self::Realization => "realization",
            Self::Relief => "relief",
            Self::SurpriseNegative => "surprise_negative",
            Self::SurprisePositive => "surprise_positive",
            Self::Sympathy => "sympathy",
            Self::Triumph => "triumph",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let key = normalize(name);
        Self::ALL.into_iter().find(|id| id.as_str() == key)
    }

    /// True for the 12 Nature Human Behaviour names. The other 12 are still
    /// legal on a spoken word; this flag only lowers the default confidence.
    pub fn reliable_in_prosody(self) -> bool {
        RELIABLE.contains(&self.as_str())
    }

    pub fn tier(self) -> ProsodyTier {
        if self.reliable_in_prosody() {
            ProsodyTier::Reliable
        } else {
            ProsodyTier::Extended
        }
    }

    /// Pipeline default on the arousal / valence / dominance point.
    /// Not a coordinate from Cowen, Russell, Banse, or Juslin.
    /// The spec agent owns cited targets (PR #189). These exist so a render
    /// has a direction to walk.
    pub fn prototype(self) -> Avd {
        let (arousal, valence, dominance) = match self {
            Self::Adoration => (0.30, 0.55, 0.35),
            Self::Amusement => (0.70, 0.60, 0.55),
            Self::Anger => (0.95, -0.70, 0.85),
            Self::Awe => (0.55, 0.40, 0.30),
            Self::Confusion => (0.50, -0.20, 0.35),
            Self::Contempt => (0.45, -0.55, 0.75),
            Self::Desire => (0.60, 0.45, 0.40),
            Self::Disappointment => (0.28, -0.45, 0.30),
            Self::Distress => (0.80, -0.75, 0.25),
            Self::Fear => (0.85, -0.65, 0.20),
            Self::Interest => (0.48, 0.25, 0.50),
            Self::Sadness => (0.02, -0.60, 0.25),
            Self::Contentment => (0.22, 0.50, 0.45),
            Self::Disgust => (0.40, -0.70, 0.60),
            Self::Ecstasy => (0.98, 0.85, 0.70),
            Self::Elation => (0.92, 0.80, 0.65),
            Self::Embarrassment => (0.90, -0.35, 0.20),
            Self::Pain => (0.75, -0.80, 0.30),
            Self::Realization => (0.52, 0.10, 0.55),
            Self::Relief => (0.32, 0.40, 0.40),
            Self::SurpriseNegative => (0.78, -0.40, 0.35),
            Self::SurprisePositive => (0.82, 0.55, 0.45),
            Self::Sympathy => (0.25, 0.15, 0.35),
            Self::Triumph => (0.93, 0.70, 0.90),
        };
        Avd {
            arousal,
            valence,
            dominance,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProsodyTier {
    Reliable,
    Extended,
}

/// Which track carries the category. Both tracks use [`EmotionId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    /// Spoken words. Any of the 24, or neutral.
    Prosody,
    /// A burst (a laugh, a cry). Any of the 24. Not the only place a feeling can sit.
    Burst,
}

impl Default for Track {
    fn default() -> Self {
        Self::Prosody
    }
}

/// Neutral is the speaker baseline. It is not a Cowen category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feeling {
    Neutral,
    Emotion(EmotionId),
}

impl Feeling {
    pub fn emotion(self) -> Option<EmotionId> {
        match self {
            Self::Neutral => None,
            Self::Emotion(id) => Some(id),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Emotion(id) => id.as_str(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedTarget {
    pub feeling: Feeling,
    pub track: Track,
    /// Set when an older name was rewritten. `laughter` becomes burst amusement.
    pub alias_from: Option<&'static str>,
}

pub fn normalize(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_")
}

/// Resolve a name. Older map, and only this map: seductive→desire,
/// tender→adoration, calm→neutral, laughter→burst amusement.
/// Unknown names fail. They are not guessed from a neighbour.
pub fn parse_target(name: &str, requested: Track) -> Result<ParsedTarget, String> {
    let key = normalize(name);
    let (feeling, track, alias) = match key.as_str() {
        "neutral" | "baseline" => (Feeling::Neutral, requested, None),
        "calm" => (Feeling::Neutral, Track::Prosody, Some("calm")),
        "seductive" => (
            Feeling::Emotion(EmotionId::Desire),
            requested,
            Some("seductive"),
        ),
        "tender" => (
            Feeling::Emotion(EmotionId::Adoration),
            requested,
            Some("tender"),
        ),
        "laughter" | "laugh" => (
            Feeling::Emotion(EmotionId::Amusement),
            Track::Burst,
            Some("laughter"),
        ),
        _ => {
            let id = EmotionId::parse(&key).ok_or_else(|| {
                format!(
                    "unknown emotion '{name}'. Use one of the 24 Cowen categories, or neutral. \
                     Older names accepted: seductive, tender, calm, laughter."
                )
            })?;
            (Feeling::Emotion(id), requested, None)
        }
    };
    Ok(ParsedTarget {
        feeling,
        track,
        alias_from: alias,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_set_of_24_and_a_tier_of_12() {
        assert_eq!(EmotionId::ALL.len(), 24);
        assert_eq!(RELIABLE.len(), 12);
        assert_eq!(EXTENDED.len(), 12);
        let mut reliable = 0;
        for id in EmotionId::ALL {
            assert_eq!(EmotionId::parse(id.as_str()), Some(id));
            if id.reliable_in_prosody() {
                reliable += 1;
                assert_eq!(id.tier(), ProsodyTier::Reliable);
                assert!(RELIABLE.contains(&id.as_str()));
            } else {
                assert_eq!(id.tier(), ProsodyTier::Extended);
                assert!(EXTENDED.contains(&id.as_str()));
            }
        }
        assert_eq!(reliable, 12);
        assert!(!EmotionId::Embarrassment.reliable_in_prosody());
        assert!(EmotionId::Anger.reliable_in_prosody());
    }

    #[test]
    fn spoken_words_accept_every_category_including_the_extended_twelve() {
        for id in EmotionId::ALL {
            let parsed = parse_target(id.as_str(), Track::Prosody).unwrap();
            assert_eq!(parsed.track, Track::Prosody);
            assert_eq!(parsed.feeling, Feeling::Emotion(id));
            assert!(parsed.alias_from.is_none());
        }
    }

    #[test]
    fn old_names_map_once() {
        let seductive = parse_target("seductive", Track::Prosody).unwrap();
        assert_eq!(seductive.feeling, Feeling::Emotion(EmotionId::Desire));
        assert_eq!(seductive.track, Track::Prosody);
        assert_eq!(seductive.alias_from, Some("seductive"));

        let tender = parse_target("tender", Track::Prosody).unwrap();
        assert_eq!(tender.feeling, Feeling::Emotion(EmotionId::Adoration));

        let calm = parse_target("calm", Track::Burst).unwrap();
        assert_eq!(calm.feeling, Feeling::Neutral);
        assert_eq!(calm.track, Track::Prosody);

        let laugh = parse_target("laughter", Track::Prosody).unwrap();
        assert_eq!(laugh.feeling, Feeling::Emotion(EmotionId::Amusement));
        assert_eq!(laugh.track, Track::Burst);

        assert!(parse_target("happy", Track::Prosody).is_err());
    }
}
