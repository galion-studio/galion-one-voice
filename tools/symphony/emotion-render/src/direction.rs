// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Direction of change, per category, relative to the speaker's own neutral.
//!
//! A sign is a published modal call or a published residual sign.
//! A missing row stays [`Sign::Unknown`]. A published disagreement stays
//! [`Sign::Unresolved`]. Neither case is filled from a neighbour.
//! Banse z values are versus the cross-emotion mean with sex partialed out
//! (Banse & Scherer 1996, Table 6, p. 625). They are not semitones and they
//! are not z against this speaker's neutral. Only the sign is read.
//! Juslin & Laukka (2003) Table 7 is the modal call, not a z.
//! Scherer (1986) was not re-read; no number is taken from it.
//! The Scherer source that was read is Banse & Scherer (1996).

use crate::inventory::Category;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Higher,
    Lower,
    Unknown,
    /// The sources disagree, or the modal call is bimodal / inconsistent.
    /// Do not pick a side.
    Unresolved,
}

#[derive(Clone, Copy, Debug)]
pub struct Feat {
    pub sign: Sign,
    pub cite: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Acoustics {
    pub pitch: Feat,
    pub pitch_range: Feat,
    pub rate: Feat,
    pub energy: Feat,
    pub pause: Feat,
    /// hot_anger, cold_anger, panic, anxiety, or the Juslin modal label.
    pub pole: Option<&'static str>,
    pub cite: &'static [&'static str],
}

pub const COWEN_AP: &str =
    "Cowen, Elfenbein, Laukka & Keltner 2019, American Psychologist, 74(6), 698-712, Figure 3B";
pub const COWEN_NHB: &str =
    "Cowen, Laukka, Elfenbein, Liu & Keltner 2019, Nature Human Behaviour, 3(4), 369-382";

const JUSLIN: &str = "Juslin & Laukka 2003, Psychological Bulletin, 129(5), Table 7, pp. 792-795";
const JUSLIN_PAUSE: &str = "Juslin & Laukka 2003, p. 796 (pause proportion)";
const BANSE: &str =
    "Banse & Scherer 1996, Journal of Personality and Social Psychology, 70(3), Table 6, p. 625";

fn feat(sign: Sign, cite: &'static str) -> Feat {
    Feat { sign, cite }
}

fn unknown_feat() -> Feat {
    Feat {
        sign: Sign::Unknown,
        cite: "",
    }
}

fn unknown_row(pole: Option<&'static str>) -> Acoustics {
    Acoustics {
        pitch: unknown_feat(),
        pitch_range: unknown_feat(),
        rate: unknown_feat(),
        energy: unknown_feat(),
        pause: unknown_feat(),
        pole,
        cite: &[COWEN_AP, COWEN_NHB],
    }
}

/// Speech-portrayal directions. Burst rendering does not apply these:
/// a wordless sound is not a shift against this speaker's speech neutral.
/// Dominance picks the published pole inside anger and inside fear.
/// The cut at zero is the caller's bipolar centre. No paper here publishes
/// a dominance coefficient in Hz or in z.
pub fn speech_directions(category: Category, dominance: f64) -> Acoustics {
    match category {
        // Juslin & Laukka 2003 Table 7, love–tenderness (they did not say adoration):
        // slow, quiet, low F0, low F0 variability, low high-frequency energy.
        // Pauses were little studied (p. 796), so pause stays unknown.
        Category::Adoration => Acoustics {
            pitch: feat(Sign::Lower, JUSLIN),
            pitch_range: feat(Sign::Lower, JUSLIN),
            rate: feat(Sign::Lower, JUSLIN),
            energy: feat(Sign::Lower, JUSLIN),
            pause: unknown_feat(),
            pole: None,
            cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE],
        },
        // Spoken amusement has no acoustic row. Juslin/Banse happiness is a
        // split with interest and is not copied here. Bachorowski, Smoski &
        // Owren 2001 measured laughs in absolute Hz, not z against the same
        // person's speech, so those numbers are not a pitch shift.
        Category::Amusement => unknown_row(None),
        Category::Anger => anger(dominance),
        // Cowen 2019 NHB dimension D ties surprise with awe. No acoustic row
        // was read for awe.
        Category::Awe => unknown_row(None),
        // Cowen 2019 NHB Figure 7 blends confusion with interest, not with
        // surprise. No acoustic row.
        Category::Confusion => unknown_row(None),
        // Banse's disgust profile is not contempt.
        Category::Contempt => unknown_row(None),
        Category::Contentment => unknown_row(None),
        // Hughes, Farley & Rhodes 2010, p. 160: both sexes lowered F0 toward
        // an attractive opposite-sex addressee. That is courtship-directed
        // speech, not a desire portrayal. The published means are absolute
        // Hz in that phone-message task (pooled 208.84 vs 215.44). They are
        // not a semitone offset for every speaker. Rate, energy and pause
        // were not reported there.
        Category::Desire => Acoustics {
            pitch: feat(
                Sign::Lower,
                "Hughes, Farley & Rhodes 2010, Journal of Nonverbal Behavior, 34(3), p. 160",
            ),
            pitch_range: unknown_feat(),
            rate: unknown_feat(),
            energy: unknown_feat(),
            pause: unknown_feat(),
            pole: None,
            cite: &[
                COWEN_AP,
                COWEN_NHB,
                "Hughes, Farley & Rhodes 2010, p. 160",
            ],
        },
        Category::Disappointment => unknown_row(None),
        // Banse & Scherer 1996 Table 6: disgust F0 z = -0.29, loudness z = -0.51
        // (sign only). Recognition 15% (p. 622). Boiten 1998 abstract: disgust
        // prolongs the inspiratory pause. That is not Juslin's pause proportion.
        Category::Disgust => Acoustics {
            pitch: feat(Sign::Lower, BANSE),
            pitch_range: unknown_feat(),
            rate: unknown_feat(),
            energy: feat(Sign::Lower, BANSE),
            pause: feat(
                Sign::Higher,
                "Boiten 1998, Biological Psychology, 49, 29-51, abstract: disgust prolongs the inspiratory pause",
            ),
            pole: None,
            cite: &[COWEN_AP, COWEN_NHB, BANSE, "Boiten 1998, abstract"],
        },
        // Not Banse despair (F0 z = +0.99, energy z = +1.00). Despair is
        // Banse's word. No distress row was read.
        Category::Distress => unknown_row(None),
        Category::Ecstasy => unknown_row(None),
        // Banse & Scherer 1996 Table 6, elation: F0 z = +1.24, loudness z = +1.05,
        // articulation-duration z = +0.12. Higher duration means slower.
        // These are speech portrayals of elation. They are not copied onto
        // interest or amusement, and Juslin's happiness pool is not used.
        Category::Elation => Acoustics {
            pitch: feat(Sign::Higher, BANSE),
            pitch_range: unknown_feat(),
            rate: feat(Sign::Lower, BANSE),
            energy: feat(Sign::Higher, BANSE),
            pause: unknown_feat(),
            pole: Some("elation"),
            cite: &[COWEN_AP, COWEN_NHB, BANSE],
        },
        // One of the 24 (Cowen 2019 AP). Not in the NHB twelve. No acoustic row.
        Category::Embarrassment => unknown_row(None),
        Category::Fear => fear(dominance),
        // No interest-specific acoustic row. Happiness numbers are not copied.
        Category::Interest => unknown_row(None),
        Category::Pain => unknown_row(None),
        Category::Realization => unknown_row(None),
        Category::Relief => unknown_row(None),
        Category::Sadness => sadness(),
        Category::SurpriseNegative => unknown_row(None),
        Category::SurprisePositive => unknown_row(None),
        Category::Sympathy => unknown_row(None),
        Category::Triumph => unknown_row(None),
    }
}

fn anger(dominance: f64) -> Acoustics {
    // Juslin & Laukka 2003 Table 7: anger is fast, loud, high F0, high F0
    // variability, high high-frequency energy, few pauses (p. 796).
    // Banse & Scherer 1996 Table 6: hot anger F0 z = +1.13, energy z = +1.19,
    // articulation-duration z = -0.31 (faster). Cold anger F0 z = +0.16,
    // energy z = +0.52, articulation-duration z = -0.14 (still faster, still
    // above the cross-emotion mean, smaller residual). Both poles keep the
    // same signs. Dominance only names the pole so a config file can give
    // them different step sizes. High-frequency energy is Juslin's alpha-ratio
    // cue; the cloning layer has no alpha-ratio knob, so it is not a control.
    let pole = if dominance > 0.0 {
        "hot_anger"
    } else if dominance < 0.0 {
        "cold_anger"
    } else {
        "anger_modal"
    };
    Acoustics {
        pitch: feat(Sign::Higher, JUSLIN),
        pitch_range: feat(Sign::Higher, JUSLIN),
        rate: feat(Sign::Higher, JUSLIN),
        energy: feat(Sign::Higher, JUSLIN),
        pause: feat(Sign::Lower, JUSLIN_PAUSE),
        pole: Some(pole),
        cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE, BANSE],
    }
}

fn fear(dominance: f64) -> Acoustics {
    // Juslin & Laukka 2003 Table 7: fear is mostly fast and high F0. Intensity
    // and high-frequency energy are bimodal. F0-variability studies fall more
    // often in the low bin; the label file records that call as inconsistent,
    // so the range is unresolved rather than taken as a single sign.
    // Pauses for fear were inconsistent (p. 796).
    // Banse & Scherer 1996 Table 6: panic F0 z = +1.23, energy z = +0.84,
    // articulation-duration z = -0.58 (faster). Anxiety F0 z = -0.58,
    // energy z = -0.37, articulation-duration z = -0.35 (still faster).
    // Dominance above the caller's zero selects panic; below selects anxiety.
    // At zero the Juslin modal is used, and loudness stays unresolved.
    if dominance > 0.0 {
        Acoustics {
            pitch: feat(Sign::Higher, BANSE),
            pitch_range: feat(Sign::Unresolved, JUSLIN),
            rate: feat(Sign::Higher, BANSE),
            energy: feat(Sign::Higher, BANSE),
            pause: feat(Sign::Unresolved, JUSLIN_PAUSE),
            pole: Some("panic"),
            cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE, BANSE],
        }
    } else if dominance < 0.0 {
        Acoustics {
            pitch: feat(Sign::Lower, BANSE),
            pitch_range: feat(Sign::Unresolved, JUSLIN),
            rate: feat(Sign::Higher, BANSE),
            energy: feat(Sign::Lower, BANSE),
            pause: feat(Sign::Unresolved, JUSLIN_PAUSE),
            pole: Some("anxiety"),
            cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE, BANSE],
        }
    } else {
        Acoustics {
            pitch: feat(Sign::Higher, JUSLIN),
            pitch_range: feat(Sign::Unresolved, JUSLIN),
            rate: feat(Sign::Higher, JUSLIN),
            energy: feat(Sign::Unresolved, JUSLIN),
            pause: feat(Sign::Unresolved, JUSLIN_PAUSE),
            pole: Some("fear_modal"),
            cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE, BANSE],
        }
    }
}

fn sadness() -> Acoustics {
    // Juslin & Laukka 2003 Table 7: slow, quiet, low F0, low F0 variability,
    // low high-frequency energy, many pauses (p. 796).
    // Banse & Scherer 1996 Table 6, sadness: F0 z = -0.32, energy z = -1.16,
    // articulation-duration z = +1.04 (slower). Banse despair is a different
    // row and is not this category and not Cowen's distress.
    Acoustics {
        pitch: feat(Sign::Lower, JUSLIN),
        pitch_range: feat(Sign::Lower, JUSLIN),
        rate: feat(Sign::Lower, JUSLIN),
        energy: feat(Sign::Lower, JUSLIN),
        pause: feat(Sign::Higher, JUSLIN_PAUSE),
        pole: Some("sadness"),
        cite: &[COWEN_AP, COWEN_NHB, JUSLIN, JUSLIN_PAUSE, BANSE],
    }
}

pub fn sign_word(feature: &str, sign: Sign) -> &'static str {
    match (feature, sign) {
        (_, Sign::Unknown) => "unknown",
        (_, Sign::Unresolved) => "unresolved",
        ("rate", Sign::Higher) => "faster",
        ("rate", Sign::Lower) => "slower",
        ("pause", Sign::Higher) => "more",
        ("pause", Sign::Lower) => "fewer",
        (_, Sign::Higher) => "higher",
        (_, Sign::Lower) => "lower",
    }
}
