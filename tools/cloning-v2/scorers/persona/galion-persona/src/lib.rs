//! Persona emotion targets and the persona-aware 0.83 gate.
//!
//! Crying decision strength is emotion2vec+ `sad`, never EfficientAT crying.
//! Categories absent from a persona file are `Na` (not 0).

pub mod persona;
pub mod strength;

pub use persona::{load_persona, EmotionSpec, Persona};
pub use strength::{decision_strength, NaOr, NA};

pub const GATE: f64 = 0.83;
pub const PRESENCE: f64 = 0.20;
pub const CATEGORY_WEIGHT: f64 = 0.70;
pub const EMB_WEIGHT: f64 = 0.30;

/// `emotion_match = 0.70 * category_blend + 0.30 * emb_sim`.
pub fn emotion_match(category_blend: f64, emb_sim: f64) -> f64 {
    CATEGORY_WEIGHT * category_blend + EMB_WEIGHT * emb_sim
}

pub fn gate_pass(emotion_match: f64, gate: f64) -> bool {
    emotion_match + 1e-12 >= gate
}

/// Strength-weighted mean of per-emotion scores (in-set only).
pub fn category_blend(pairs: &[(f64, f64)]) -> Option<f64> {
    let mut num = 0.0;
    let mut den = 0.0;
    for &(strength, score) in pairs {
        let w = strength.max(1e-12);
        num += w * score;
        den += w;
    }
    if den > 0.0 {
        Some(num / den)
    } else {
        None
    }
}

pub fn score_c(matches: &[f64]) -> Option<f64> {
    if matches.is_empty() {
        return None;
    }
    let max = matches.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean = matches.iter().sum::<f64>() / matches.len() as f64;
    Some(0.55 * max + 0.45 * mean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona::load_persona;
    use crate::strength::{decision_strength, e2v_sad_crying, NaOr};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn personas_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("pipeline/personas")
    }

    #[test]
    fn samantha_includes_seductive_excludes_yelling() {
        let p = load_persona(&personas_dir().join("own-samantha.toml")).unwrap();
        assert_eq!(p.id, "own-samantha");
        assert!((p.gate - 0.83).abs() < 1e-15);
        assert!(p.in_set("seductive"));
        assert!(p.in_set("happy"));
        assert!(p.in_set("joyful"));
        assert!(!p.in_set("yelling"));
        assert!(!p.in_set("crying"));
        let agg = p.spec_for("aggressive").unwrap();
        assert!((agg.target.unwrap() - 0.9990363121032715).abs() < 1e-15);
    }

    #[test]
    fn jarvis_seductive_is_na() {
        let p = load_persona(&personas_dir().join("own-jarvis.toml")).unwrap();
        assert_eq!(p.id, "own-jarvis");
        assert!(!p.in_set("seductive"));
        assert!(!p.in_set("love"));
        assert!(p.in_set("neutral"));
        assert!(p.in_set("logic"));
        assert!(p.in_set("reason"));
        let mut e2v = BTreeMap::new();
        e2v.insert("sad".into(), 0.9);
        e2v.insert("neutral".into(), 1.0);
        let mut clap = BTreeMap::new();
        clap.insert("seductive".into(), 0.5);
        clap.insert("logic".into(), 0.8);
        let mut eat = BTreeMap::new();
        eat.insert("crying".into(), 0.3);
        eat.insert("yelling".into(), 0.7);
        assert_eq!(
            decision_strength("seductive", &e2v, &clap, &eat, Some(&p)),
            NaOr::Na
        );
        assert_eq!(
            decision_strength("love", &e2v, &clap, &eat, Some(&p)),
            NaOr::Na
        );
        match decision_strength("logic", &e2v, &clap, &eat, Some(&p)) {
            NaOr::Value(v) => assert!((v - 0.8).abs() < 1e-15),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn crying_is_e2v_sad_not_efficientat() {
        let mut e2v = BTreeMap::new();
        e2v.insert("sad".into(), 0.9999972581863403);
        let mut eat = BTreeMap::new();
        eat.insert("crying".into(), 0.12454459071159363);
        let v = e2v_sad_crying(&e2v);
        assert!((v - 0.9999972581863403).abs() < 1e-15);
        assert!((v - eat["crying"]).abs() > 0.8);
        // without a persona, crying is scored (for diagnostics / other voices)
        match decision_strength("crying", &e2v, &BTreeMap::new(), &eat, None) {
            NaOr::Value(x) => assert!((x - 0.9999972581863403).abs() < 1e-15),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn gate_uses_published_threshold() {
        assert!(gate_pass(0.83, GATE));
        assert!(!gate_pass(0.829, GATE));
        let blend = category_blend(&[(1.0, 0.9), (0.5, 0.8)]).unwrap();
        // (1.0*0.9 + 0.5*0.8) / 1.5 = 1.3/1.5
        assert!((blend - (1.3 / 1.5)).abs() < 1e-15);
        let em = emotion_match(blend, 1.0);
        assert!((em - (0.70 * blend + 0.30)).abs() < 1e-15);
    }
}
