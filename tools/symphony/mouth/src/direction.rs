// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Mouth directions relative to the speaker's own neutral speech.
//!
//! A sign is a published call. A missing row is not filled from a neighbour.
//! Banse z and Tartter's per-speaker hertz are not copied. Juslin's happiness
//! F1 is not assigned to amusement or to interest.
//! Burst rendering does not apply these speech signs.

use crate::inventory::Category;

pub const JUSLIN_F1: &str = "Juslin & Laukka 2003, Psychological Bulletin 129(5), p. 796: F1 higher in anger and happiness, lower in fear and sadness (preliminary). Happiness is not a Cowen category and is not copied onto amusement or interest.";
pub const JUSLIN_RATE: &str = "Juslin & Laukka 2003, Table 7, pp. 792-795";
pub const BANSE: &str = "Banse & Scherer 1996, Journal of Personality and Social Psychology 70(3), Table 6, p. 625. Sign of articulation duration only. Higher duration is slower. The z is not a rate multiplier.";
pub const BANSE_ELATION: &str = "Banse & Scherer 1996, Table 6, elation articulation-duration z is positive, so the sign is slower. Not copied onto amusement or interest. The z itself is not a multiplier.";
pub const TARTTER: &str = "Tartter 1980, Perception & Psychophysics 27:24-27: smiling raises F0 and F1-F3 against straight-faced speech. Per-speaker Hz were not copied. Frowning is not an intent in this stub.";
pub const GARNIER: &str = "Garnier, Henrich & Dubois 2010, JSLHR 53:588-608, Experiment 2: lip opening and F1 rose from quiet to noise when addressing someone. The figure was not digitised; the delta is unknown.";
pub const LINDBLOM_SUNDBERG: &str = "Lindblom & Sundberg 1971, JASA 50(4B):1166-1179: jaw opening raises F1, tongue fronting raises F2, lip rounding lowers formants. No hertz from their figures is copied.";
pub const OHMAN: &str = "Ohman 1966, JASA 39:151-168: vowels coarticulate through an intervening consonant. The blend weight is config, not a measured locus.";
pub const LINDBLOM_1963: &str = "Lindblom 1963, JASA 35:1773-1781: shorter vowels undershoot their targets. Gay 1978, JASA 63:223-230, found transition duration relatively stable. Rate does not automatically change the coarticulation weight.";
pub const HH: &str = "Lindblom 1990, H&H theory, in Hardcastle & Marchal (eds.), Speech Production and Speech Modelling: hyperarticulation reaches targets, hypoarticulation undershoots. Moon & Lindblom 1994, JASA 96:40-55: clear speech reduces context-induced formant displacement. The amount is config.";
pub const RHUBARB: &str = "Rhubarb Lip Sync 1.13.0 (MIT), README mouth shapes A-H and X. The phone-to-shape table is that recogniser's set, not a new viseme inventory.";
pub const BACHOROWSKI: &str = "Bachorowski, Smoski & Owren 2001, JASA 110:1581-1597: both open-mouth and closed-mouth laughs occur. A burst of amusement does not force one jaw target. Absolute laugh formants are not speech shifts.";

/// +1 higher / faster / more open, -1 lower / slower / more closed, 0 no row.
pub fn f1_sign(category: Category) -> i8 {
    match category {
        Category::Anger => 1,
        Category::Fear | Category::Sadness => -1,
        _ => 0,
    }
}

pub fn f1_cite(category: Category) -> &'static str {
    if f1_sign(category) == 0 {
        ""
    } else {
        JUSLIN_F1
    }
}

pub fn rate_sign(category: Category) -> i8 {
    match category {
        Category::Anger | Category::Fear => 1,
        Category::Sadness | Category::Adoration | Category::Elation => -1,
        _ => 0,
    }
}

pub const JUSLIN_AND_BANSE_RATE: &str = "Juslin & Laukka 2003, Table 7: anger and fear are faster, sadness is slower. Banse & Scherer 1996, Table 6: hot and cold anger, panic, and anxiety have negative articulation-duration z (faster); sadness has positive z (slower). The z is not a multiplier.";

pub fn rate_cite(category: Category) -> &'static str {
    match category {
        Category::Anger | Category::Fear | Category::Sadness => JUSLIN_AND_BANSE_RATE,
        Category::Adoration => JUSLIN_RATE,
        Category::Elation => BANSE_ELATION,
        _ => "",
    }
}

/// Pole name for a config override. The cited sign does not change with the pole.
/// Hot and cold anger are both faster and both higher in F1. Panic and anxiety
/// are both faster. Anxiety's F0 sign differs; F1 was not split in the sources read.
pub fn pole(category: Category, dominance: f64) -> Option<&'static str> {
    match category {
        Category::Anger if dominance > 0.0 => Some("hot_anger"),
        Category::Anger if dominance < 0.0 => Some("cold_anger"),
        Category::Anger => Some("anger_modal"),
        Category::Fear if dominance > 0.0 => Some("panic"),
        Category::Fear if dominance < 0.0 => Some("anxiety"),
        Category::Fear => Some("fear_modal"),
        Category::Elation => Some("elation"),
        Category::Sadness => Some("sadness"),
        _ => None,
    }
}

pub fn sign_word(sign: i8, positive: &str, negative: &str) -> String {
    match sign.cmp(&0) {
        std::cmp::Ordering::Greater => positive.to_string(),
        std::cmp::Ordering::Less => negative.to_string(),
        std::cmp::Ordering::Equal => "not_applied".to_string(),
    }
}
