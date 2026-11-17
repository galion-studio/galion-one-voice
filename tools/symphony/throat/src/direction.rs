// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Direction of change, relative to the speaker's own neutral.
//!
//! A sign is a published modal call or a published residual sign.
//! A missing row stays [`Sign::Unknown`]. A published disagreement stays
//! [`Sign::Unresolved`]. Neither is filled from a neighbour.
//! Banse z values are versus the cross-emotion mean with sex partialed out.
//! They are not semitones and they are not Rd.
//! High-frequency energy is Juslin's cue. It is the sign for the Rd control,
//! because a lower Rd raises the glottal cutoff Fa (Fant and Kruckenberg
//! 1996). The size of that Rd step is config, and the shipped value is unknown.
//! Burst rendering does not apply these signs.

use crate::inventory::Category;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Higher,
    Lower,
    Unknown,
    Unresolved,
}

impl Sign {
    pub fn as_str(self) -> &'static str {
        match self {
            Sign::Higher => "higher",
            Sign::Lower => "lower",
            Sign::Unknown => "unknown",
            Sign::Unresolved => "unresolved",
        }
    }

    /// +1 higher, -1 lower, None when the sign must not move a knob.
    pub fn pole(self) -> Option<f64> {
        match self {
            Sign::Higher => Some(1.0),
            Sign::Lower => Some(-1.0),
            Sign::Unknown | Sign::Unresolved => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Acoustics {
    pub pitch: Sign,
    pub pitch_range: Sign,
    /// High-frequency energy. Applied, when a scale exists, as the opposite
    /// sign on Rd: more high-frequency energy, lower Rd, higher Fa.
    pub hf: Sign,
    pub pole: Option<&'static str>,
    pub pitch_cite: &'static str,
    pub hf_cite: &'static str,
}

const JUSLIN: &str = "Juslin & Laukka 2003, Psychological Bulletin, 129(5), Table 7, pp. 792-795";
const JUSLIN_HF: &str = "Juslin & Laukka 2003, Table 7: high-frequency energy";
const BANSE: &str =
    "Banse & Scherer 1996, Journal of Personality and Social Psychology, 70(3), Table 6, p. 625";
const HUGHES: &str = "Hughes, Farley & Rhodes 2010, Journal of Nonverbal Behavior, 34(3), p. 160";

fn row(
    pitch: Sign,
    pitch_range: Sign,
    hf: Sign,
    cite: &'static str,
    hf_cite: &'static str,
) -> Acoustics {
    Acoustics {
        pitch,
        pitch_range,
        hf,
        pole: None,
        pitch_cite: cite,
        hf_cite,
    }
}

fn unknown() -> Acoustics {
    row(Sign::Unknown, Sign::Unknown, Sign::Unknown, "", "")
}

pub fn speech_directions(category: Category, dominance: f64) -> Acoustics {
    match category {
        // Juslin love–tenderness, which this inventory calls adoration:
        // low F0, low F0 variability, low high-frequency energy.
        Category::Adoration => row(Sign::Lower, Sign::Lower, Sign::Lower, JUSLIN, JUSLIN_HF),
        Category::Amusement => unknown(),
        Category::Anger => anger(dominance),
        Category::Awe => unknown(),
        Category::Confusion => unknown(),
        Category::Contempt => unknown(),
        Category::Contentment => unknown(),
        Category::Desire => row(Sign::Lower, Sign::Unknown, Sign::Unknown, HUGHES, ""),
        Category::Disappointment => unknown(),
        Category::Disgust => row(Sign::Lower, Sign::Unknown, Sign::Unknown, BANSE, ""),
        Category::Distress => unknown(),
        Category::Ecstasy => unknown(),
        Category::Elation => {
            let mut a = row(Sign::Higher, Sign::Unknown, Sign::Unknown, BANSE, "");
            a.pole = Some("elation");
            a
        }
        Category::Embarrassment => unknown(),
        Category::Fear => fear(dominance),
        Category::Interest => unknown(),
        Category::Pain => unknown(),
        Category::Realization => unknown(),
        Category::Relief => unknown(),
        Category::Sadness => {
            let mut a = row(Sign::Lower, Sign::Lower, Sign::Lower, JUSLIN, JUSLIN_HF);
            a.pole = Some("sadness");
            a
        }
        Category::SurpriseNegative => unknown(),
        Category::SurprisePositive => unknown(),
        Category::Sympathy => unknown(),
        Category::Triumph => unknown(),
    }
}

fn anger(dominance: f64) -> Acoustics {
    // Juslin: high F0, high F0 variability, high high-frequency energy.
    // Banse hot and cold anger keep those signs; dominance only names the pole
    // so a config file can give them different step sizes. Both poles are
    // still above the cross-emotion mean in Table 6.
    let pole = if dominance > 0.0 {
        "hot_anger"
    } else if dominance < 0.0 {
        "cold_anger"
    } else {
        "anger_modal"
    };
    Acoustics {
        pitch: Sign::Higher,
        pitch_range: Sign::Higher,
        hf: Sign::Higher,
        pole: Some(pole),
        pitch_cite: JUSLIN,
        hf_cite: JUSLIN_HF,
    }
}

fn fear(dominance: f64) -> Acoustics {
    // Juslin: mostly high F0. High-frequency energy is bimodal, so it is not
    // a sign. F0 variability is unresolved (p. 796 and Table 7).
    // Banse: panic F0 above the cross-emotion mean, anxiety F0 below it.
    // Dominance above zero selects panic; below selects anxiety.
    if dominance > 0.0 {
        Acoustics {
            pitch: Sign::Higher,
            pitch_range: Sign::Unresolved,
            hf: Sign::Unresolved,
            pole: Some("panic"),
            pitch_cite: BANSE,
            hf_cite: JUSLIN_HF,
        }
    } else if dominance < 0.0 {
        Acoustics {
            pitch: Sign::Lower,
            pitch_range: Sign::Unresolved,
            hf: Sign::Unresolved,
            pole: Some("anxiety"),
            pitch_cite: BANSE,
            hf_cite: JUSLIN_HF,
        }
    } else {
        Acoustics {
            pitch: Sign::Higher,
            pitch_range: Sign::Unresolved,
            hf: Sign::Unresolved,
            pole: Some("fear_modal"),
            pitch_cite: JUSLIN,
            hf_cite: JUSLIN_HF,
        }
    }
}
