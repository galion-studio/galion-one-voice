use crate::persona::Persona;
use std::collections::BTreeMap;

pub const NA: &str = "n/a";

#[derive(Debug, Clone, PartialEq)]
pub enum NaOr {
    Na,
    Value(f64),
}

/// Crying decision = emotion2vec+ `sad`. EfficientAT crying is not consulted.
pub fn e2v_sad_crying(e2v_probs: &BTreeMap<String, f64>) -> f64 {
    e2v_probs.get("sad").copied().unwrap_or(0.0)
}

fn trained_key(cat: &str) -> Option<&'static str> {
    match cat {
        "joyful" | "happy" => Some("happy"),
        "aggressive" => Some("angry"),
        "disgust" => Some("disgusted"),
        "fear" => Some("fearful"),
        "sad" => Some("sad"),
        "neutral" => Some("neutral"),
        "surprise" => Some("surprised"),
        _ => None,
    }
}

/// Decision strength for one category.
///
/// * `crying` → emotion2vec+ sad (never EfficientAT crying)
/// * trained classes → emotion2vec+ mapped class
/// * yelling / laughter → EfficientAT max
/// * love / seductive / logic / reason → CLAP probs (legacy fallback;
///   a love_seductive plugin replaces the Python path, not this Rust helper)
///
/// If `persona` is Some and the category is not in that persona, returns `Na`.
pub fn decision_strength(
    category: &str,
    e2v_probs: &BTreeMap<String, f64>,
    clap_probs: &BTreeMap<String, f64>,
    eat_max: &BTreeMap<String, f64>,
    persona: Option<&Persona>,
) -> NaOr {
    if let Some(p) = persona {
        if !p.in_set(category) {
            return NaOr::Na;
        }
    }
    let value = if category == "crying" {
        e2v_sad_crying(e2v_probs)
    } else if category == "yelling" || category == "laughter" {
        eat_max.get(category).copied().unwrap_or(0.0)
    } else if let Some(key) = trained_key(category) {
        e2v_probs.get(key).copied().unwrap_or(0.0)
    } else {
        clap_probs.get(category).copied().unwrap_or(0.0)
    };
    NaOr::Value(value)
}
