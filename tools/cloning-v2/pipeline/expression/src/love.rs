//! Love/warmth diagnostic for the persona scorer.
//!
//! Samantha's rescore puts love at 0.9390544380026881 on the original moment
//! and 0 on every clone line, so the match is 0.06094556199731194 and the
//! 0.83 gate fails (merge-v2 0.6869450087862239, coherent 0.6713695158531341).
//! The love plugin writes 0 whenever the mean CLAP cosine against the love
//! prompt bank is at or below 0.20. That hard zero can be a real embedding
//! loss or a detector clip. This module separates the two when a prompt-bank
//! margin, a prosody delta, or both are present. With neither, the verdict
//! is `insufficient_evidence`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const GATE: f64 = 0.83;
pub const ALIGN_MIN: f64 = 0.20;
pub const CONTRAST_TEMP: f64 = 20.0;
pub const CATEGORY_WEIGHT: f64 = 0.70;
pub const EMB_WEIGHT: f64 = 0.30;

/// Reference love moment (separation file, Samantha window w03).
pub const REF_LOVE_MEAN_COS: f64 = 0.27918557822704315;
pub const REF_NEU_MEAN_COS: f64 = 0.15484974160790443;
/// Gender-conditioned contrast before the align gate, same window.
pub const REF_LOVE_CONTRAST: f64 = 0.9232053512657916;
/// Align-gated love on that window file (Agent 2 measurement).
pub const REF_LOVE_WINDOW_DECISION: f64 = 0.9228583200845187;
/// Decision that entered the persona gate (rescore of the same moment).
pub const REF_LOVE_GATE_DECISION: f64 = 0.9390544380026881;
pub const CLONE_LOVE_MATCH: f64 = 0.06094556199731194;
pub const MERGE_V2_EMOTION_MATCH: f64 = 0.6869450087862239;
pub const COHERENT_EMOTION_MATCH: f64 = 0.6713695158531341;

pub const MARGIN_COLLAPSE: f64 = 0.04;
pub const COSINE_NEAR: f64 = 0.03;
pub const COSINE_COLLAPSE: f64 = 0.05;
pub const TILT_BRIGHTER_DB_OCT: f64 = 1.5;
pub const ENERGY_LOUDER_DB: f64 = 3.0;
pub const F0_FLATTER_ST: f64 = 0.8;
pub const SAD_GATE: f64 = 0.5;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProsodyView {
    #[serde(default)]
    pub f0_mean_hz: Option<f64>,
    #[serde(default)]
    pub f0_sd_st: Option<f64>,
    #[serde(default)]
    pub rms_db: Option<f64>,
    #[serde(default)]
    pub spectral_tilt_db_oct: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub label: String,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub decision: Option<f64>,
    #[serde(default)]
    pub match_score: Option<f64>,
    #[serde(default)]
    pub love_mean_cos: Option<f64>,
    #[serde(default)]
    pub neutral_mean_cos: Option<f64>,
    #[serde(default)]
    pub e2v_sad: Option<f64>,
    #[serde(default)]
    pub prosody: Option<ProsodyView>,
}

impl Observation {
    pub fn margin(&self) -> Option<f64> {
        Some(self.love_mean_cos? - self.neutral_mean_cos?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlendRow {
    pub category: String,
    pub strength: f64,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateSet {
    pub id: String,
    pub emb_sim: f64,
    pub emotion_match: f64,
    pub rows: Vec<BlendRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnoseReport {
    pub schema: String,
    pub label: String,
    pub verdict: String,
    pub evidence: Vec<String>,
    pub detail: String,
    pub reference_decision: f64,
    pub clone_decision: f64,
    pub match_score: f64,
    pub margin_reference: Option<f64>,
    pub margin_clone: Option<f64>,
    pub prosody_cues: Vec<String>,
}

pub fn category_blend(rows: &[BlendRow]) -> Option<f64> {
    let mut num = 0.0;
    let mut den = 0.0;
    for row in rows {
        let w = row.strength.max(1e-12);
        num += w * row.score;
        den += w;
    }
    if den > 0.0 {
        Some(num / den)
    } else {
        None
    }
}

pub fn emotion_match(rows: &[BlendRow], emb_sim: f64) -> Option<f64> {
    category_blend(rows).map(|cat| CATEGORY_WEIGHT * cat + EMB_WEIGHT * emb_sim)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoveAttribution {
    pub emotion_match: f64,
    pub pass: bool,
    /// Love row replaced with a perfect match against the reference decision.
    pub emotion_match_if_love_matched: f64,
    pub pass_if_love_matched: bool,
    /// Love row removed from the blend.
    pub emotion_match_without_love: f64,
    pub love_score: f64,
    pub love_strength: f64,
}

pub fn attribute_love(rows: &[BlendRow], emb_sim: f64, gate: f64) -> Option<LoveAttribution> {
    let actual = emotion_match(rows, emb_sim)?;
    let love = rows.iter().find(|r| r.category == "love")?;
    let perfect: Vec<BlendRow> = rows
        .iter()
        .map(|r| {
            if r.category == "love" {
                BlendRow {
                    category: r.category.clone(),
                    strength: r.strength,
                    score: 1.0,
                }
            } else {
                r.clone()
            }
        })
        .collect();
    let dropped: Vec<BlendRow> = rows.iter().filter(|r| r.category != "love").cloned().collect();
    let with_love = emotion_match(&perfect, emb_sim)?;
    let without = emotion_match(&dropped, emb_sim)?;
    Some(LoveAttribution {
        emotion_match: actual,
        pass: actual + 1e-12 >= gate,
        emotion_match_if_love_matched: with_love,
        pass_if_love_matched: with_love + 1e-12 >= gate,
        emotion_match_without_love: without,
        love_score: love.score,
        love_strength: love.strength,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProsodyState {
    Unknown,
    Held,
    Partial,
    Lost,
}

fn prosody_state(reference: &ProsodyView, clone: &ProsodyView) -> (ProsodyState, Vec<String>) {
    let mut cues = Vec::new();
    let mut saw = false;
    if let (Some(r), Some(c)) = (reference.spectral_tilt_db_oct, clone.spectral_tilt_db_oct) {
        saw = true;
        if c - r > TILT_BRIGHTER_DB_OCT {
            cues.push("spectral_tilt_brighter".into());
        }
    }
    if let (Some(r), Some(c)) = (reference.rms_db, clone.rms_db) {
        saw = true;
        if c - r > ENERGY_LOUDER_DB {
            cues.push("energy_louder".into());
        }
    }
    if let (Some(r), Some(c)) = (reference.f0_sd_st, clone.f0_sd_st) {
        saw = true;
        if r - c > F0_FLATTER_ST {
            cues.push("f0_contour_flatter".into());
        }
    }
    if !saw {
        return (ProsodyState::Unknown, cues);
    }
    let tilt = cues.iter().any(|c| c == "spectral_tilt_brighter");
    let state = if cues.is_empty() {
        ProsodyState::Held
    } else if tilt || cues.len() >= 2 {
        ProsodyState::Lost
    } else {
        ProsodyState::Partial
    };
    (state, cues)
}

fn resolve_decision(reference: &Observation, clone: &Observation) -> f64 {
    if let Some(d) = clone.decision {
        return d;
    }
    if let (Some(m), Some(r)) = (clone.match_score, reference.decision) {
        let implied = r - (1.0 - m);
        if implied >= -1e-6 {
            return implied.max(0.0);
        }
    }
    f64::NAN
}

pub fn diagnose(reference: &Observation, clone: &Observation) -> DiagnoseReport {
    let reference_decision = reference.decision.unwrap_or(REF_LOVE_GATE_DECISION);
    let clone_decision = resolve_decision(reference, clone);
    let match_score = clone.match_score.unwrap_or_else(|| {
        if clone_decision.is_finite() {
            (1.0 - (clone_decision - reference_decision).abs()).max(0.0)
        } else {
            f64::NAN
        }
    });
    let margin_reference = reference.margin();
    let margin_clone = clone.margin();
    let (prosody_state, prosody_cues) = match (&reference.prosody, &clone.prosody) {
        (Some(r), Some(c)) => prosody_state(r, c),
        _ => (ProsodyState::Unknown, Vec::new()),
    };
    let hard_zero = clone_decision.is_finite() && clone_decision <= 1e-6 && reference_decision >= 0.5;
    let align_pass = clone.love_mean_cos.map(|c| c > ALIGN_MIN).unwrap_or(false);
    let align_fail = clone.love_mean_cos.map(|c| c <= ALIGN_MIN).unwrap_or(false);
    let margin_collapsed = match (margin_reference, margin_clone) {
        (Some(r), Some(c)) => r - c > MARGIN_COLLAPSE,
        _ => false,
    };
    let margin_held = match (margin_reference, margin_clone) {
        (Some(r), Some(c)) => c + MARGIN_COLLAPSE >= r,
        _ => false,
    };
    let cos_near = match (reference.love_mean_cos, clone.love_mean_cos) {
        (Some(r), Some(c)) => (r - c).abs() <= COSINE_NEAR,
        _ => false,
    };
    let cos_collapsed = match (reference.love_mean_cos, clone.love_mean_cos) {
        (Some(r), Some(c)) => r - c > COSINE_COLLAPSE,
        _ => false,
    };
    let has_margin = margin_reference.is_some() && margin_clone.is_some();
    let decision_gap = if clone_decision.is_finite() {
        reference_decision - clone_decision
    } else {
        f64::NAN
    };

    let (verdict, mut evidence, detail) = if clone
        .e2v_sad
        .map(|s| s > SAD_GATE)
        .unwrap_or(false)
        && align_pass
        && hard_zero
    {
        (
            "detector_miss",
            vec!["sad_gate".into(), "align_would_pass".into()],
            "CLAP still clears the 0.20 love-align floor, and emotion2vec+ sad multiplied the contrast down to zero.".into(),
        )
    } else if align_pass && hard_zero {
        (
            "detector_miss",
            vec!["decision_zeroed_above_align".into()],
            "Mean love cosine is above 0.20, so the align gate would keep the contrast. The stored decision is zero.".into(),
        )
    } else if margin_held && hard_zero {
        (
            "detector_miss",
            vec!["margin_held".into(), "hard_zero".into()],
            "The love-minus-neutral prompt margin still matches the reference, and the decision was clipped to zero.".into(),
        )
    } else if cos_near && hard_zero {
        (
            "detector_miss",
            vec!["cosine_near_reference".into(), "hard_zero".into()],
            "Love-bank cosine stayed next to the reference cosine, and the decision was clipped to zero.".into(),
        )
    } else if prosody_state == ProsodyState::Held && (margin_collapsed || align_fail || cos_collapsed)
    {
        (
            "detector_miss",
            vec!["prosody_held".into()],
            "F0 contour, level, and spectral tilt still match the reference warmth. The prompt-bank score moved.".into(),
        )
    } else if (margin_collapsed || cos_collapsed) && prosody_state == ProsodyState::Lost {
        (
            "real_loss",
            vec!["clap_margin".into(), "prosody".into()],
            "The love prompt margin collapsed and the voice colour moved off the reference (brighter tilt, louder level, or a flatter F0 contour).".into(),
        )
    } else if (margin_collapsed || cos_collapsed) && prosody_state == ProsodyState::Partial {
        (
            "mixed",
            vec!["clap_margin".into(), "prosody_partial".into()],
            "The prompt margin collapsed and only one non-tilt prosody cue moved. That is not enough to call it purely acoustic or purely a detector clip.".into(),
        )
    } else if (margin_collapsed || cos_collapsed) && prosody_state == ProsodyState::Unknown {
        (
            "real_loss",
            vec!["clap_margin".into()],
            "The love prompt-bank margin collapsed against the reference. The align gate's hard zero follows that embedding gap. Prosody was not measured.".into(),
        )
    } else if prosody_state == ProsodyState::Lost && !has_margin {
        (
            "real_loss",
            vec!["prosody".into()],
            "Voice colour left the reference warmth. Prompt-bank cosines were not measured.".into(),
        )
    } else if prosody_state == ProsodyState::Held && !has_margin && hard_zero {
        (
            "detector_miss",
            vec!["prosody_held".into(), "hard_zero".into()],
            "Prosody still matches the reference and the love decision is zero. Prompt-bank cosines were not measured.".into(),
        )
    } else if !has_margin && prosody_state == ProsodyState::Unknown && hard_zero {
        (
            "insufficient_evidence",
            vec!["hard_zero".into()],
            "The clone love decision is 0 against a reference near 0.94, which is what the 0.20 align gate writes. Pass love/neutral cosines or prosody to separate a real loss from that clip.".into(),
        )
    } else if decision_gap.is_finite() && decision_gap.abs() <= 0.05 {
        (
            "held",
            vec!["decision_close".into()],
            "Clone love decision stays within 0.05 of the reference.".into(),
        )
    } else {
        (
            "mixed",
            vec!["unclassified".into()],
            "The measurements disagree. Inspect the margin and the prosody cues together.".into(),
        )
    };

    if align_fail {
        evidence.push("align_gate".into());
    }
    if hard_zero && !evidence.iter().any(|e| e == "hard_zero") {
        evidence.push("hard_zero".into());
    }

    DiagnoseReport {
        schema: "galion-love-warmth-diagnostic/1".into(),
        label: clone.label.clone(),
        verdict: verdict.into(),
        evidence,
        detail,
        reference_decision,
        clone_decision: if clone_decision.is_finite() {
            clone_decision
        } else {
            0.0
        },
        match_score: if match_score.is_finite() { match_score } else { 0.0 },
        margin_reference,
        margin_clone,
        prosody_cues,
    }
}

pub fn published_reference() -> Observation {
    Observation {
        label: "samantha-w03-love".into(),
        persona: Some("own-samantha".into()),
        decision: Some(REF_LOVE_GATE_DECISION),
        match_score: None,
        love_mean_cos: Some(REF_LOVE_MEAN_COS),
        neutral_mean_cos: Some(REF_NEU_MEAN_COS),
        e2v_sad: Some(0.0003758981474675238),
        prosody: None,
    }
}

pub fn published_clone(label: &str) -> Observation {
    Observation {
        label: label.into(),
        persona: Some("own-samantha".into()),
        decision: Some(0.0),
        match_score: Some(CLONE_LOVE_MATCH),
        love_mean_cos: None,
        neutral_mean_cos: None,
        e2v_sad: None,
        prosody: None,
    }
}

/// Full report for the committed gate case, including Jarvis (his gate passes;
/// love is outside his judged set).
pub fn diagnose_gate_document(doc: &Value) -> Value {
    let reference: Observation = serde_json::from_value(doc["reference"].clone())
        .unwrap_or_else(|_| published_reference());
    let clones = doc["clones"].as_array().cloned().unwrap_or_default();
    let mut clone_reports = Vec::new();
    for c in clones {
        let obs: Observation = match serde_json::from_value(c.clone()) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let report = diagnose(&reference, &obs);
        let mut v = serde_json::to_value(&report).unwrap_or(json!({}));
        if let Some(sets) = doc.get("gate_sets").and_then(|s| s.as_array()) {
            if let Some(set) = sets.iter().find(|s| s.get("id").and_then(|i| i.as_str()) == Some(obs.label.as_str())) {
                if let Ok(rows) = serde_json::from_value::<Vec<BlendRow>>(set["rows"].clone()) {
                    let emb = set["emb_sim"].as_f64().unwrap_or(0.0);
                    if let Some(attr) = attribute_love(&rows, emb, GATE) {
                        v["attribution"] = serde_json::to_value(&attr).unwrap_or(json!({}));
                        v["published_emotion_match"] = set["emotion_match"].clone();
                    }
                }
            }
        }
        clone_reports.push(v);
    }
    let jarvis = doc.get("jarvis").cloned().unwrap_or(json!({}));
    json!({
        "schema": "galion-love-warmth-case-report/1",
        "gate": GATE,
        "align_min": ALIGN_MIN,
        "reference_decision": reference.decision,
        "reference_window_decision": REF_LOVE_WINDOW_DECISION,
        "reference_contrast": REF_LOVE_CONTRAST,
        "reference_love_mean_cos": REF_LOVE_MEAN_COS,
        "reference_margin": REF_LOVE_MEAN_COS - REF_NEU_MEAN_COS,
        "legacy_toml_love_target": 0.8109382317899566,
        "toml_edited": false,
        "clones": clone_reports,
        "jarvis": jarvis_note(&jarvis),
        "excluded": ["crying", "yelling"],
        "seductive": "samantha-only",
    })
}

fn jarvis_note(jarvis: &Value) -> Value {
    let sets = jarvis.get("sets").and_then(|s| s.as_array());
    let mut passes = Vec::new();
    let mut all_pass = true;
    if let Some(sets) = sets {
        for set in sets {
            let em = set.get("emotion_match").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let pass = em + 1e-12 >= GATE;
            if !pass {
                all_pass = false;
            }
            passes.push(json!({
                "id": set.get("id"),
                "emotion_match": em,
                "pass": pass,
            }));
        }
    }
    let judged = jarvis
        .get("judged")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
        .unwrap_or_default();
    let love_in_set = judged.iter().any(|e| e == "love" || e == "seductive");
    json!({
        "behaviour": "unchanged",
        "gate_pass": all_pass && !passes.is_empty(),
        "sets": passes,
        "judged": judged,
        "love_in_judged_set": love_in_set,
        "seductive": "n/a",
        "note": "Jarvis stays on neutral and logic. Love and seductive stay outside his judged set, so this layer does not retune him.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(tilt: f64, rms: f64, sd: f64) -> ProsodyView {
        ProsodyView {
            f0_mean_hz: Some(180.0),
            f0_sd_st: Some(sd),
            rms_db: Some(rms),
            spectral_tilt_db_oct: Some(tilt),
        }
    }

    fn ref_obs() -> Observation {
        let mut o = published_reference();
        o.prosody = Some(view(-8.0, -22.0, 2.4));
        o
    }

    #[test]
    fn published_match_is_one_minus_reference_decision() {
        let m = 1.0 - REF_LOVE_GATE_DECISION;
        assert!((m - CLONE_LOVE_MATCH).abs() < 1e-15, "{m}");
        let report = diagnose(&published_reference(), &published_clone("merge-v2"));
        assert_eq!(report.verdict, "insufficient_evidence");
        assert!((report.clone_decision - 0.0).abs() < 1e-15);
        assert!((report.match_score - CLONE_LOVE_MATCH).abs() < 1e-15);
        assert!((report.reference_decision - REF_LOVE_GATE_DECISION).abs() < 1e-15);
    }

    #[test]
    fn margin_held_with_zero_decision_is_a_detector_miss() {
        let mut clone = published_clone("synthetic");
        clone.love_mean_cos = Some(REF_LOVE_MEAN_COS - 0.01);
        clone.neutral_mean_cos = Some(REF_NEU_MEAN_COS - 0.01);
        clone.decision = Some(0.0);
        let report = diagnose(&published_reference(), &clone);
        assert_eq!(report.verdict, "detector_miss");
    }

    #[test]
    fn collapsed_margin_and_brighter_tilt_is_real_loss() {
        let mut clone = published_clone("synthetic");
        clone.love_mean_cos = Some(0.04);
        clone.neutral_mean_cos = Some(0.02);
        clone.decision = Some(0.0);
        clone.prosody = Some(view(-8.0 + 4.0, -22.0 + 6.0, 2.4));
        let report = diagnose(&ref_obs(), &clone);
        assert_eq!(report.verdict, "real_loss");
        assert!(report.evidence.iter().any(|e| e == "clap_margin"));
        assert!(report.evidence.iter().any(|e| e == "prosody"));
    }

    #[test]
    fn collapsed_margin_with_held_prosody_is_a_detector_miss() {
        let mut clone = published_clone("synthetic");
        clone.love_mean_cos = Some(0.05);
        clone.neutral_mean_cos = Some(0.03);
        clone.decision = Some(0.0);
        clone.prosody = Some(view(-8.0, -22.0, 2.4));
        let report = diagnose(&ref_obs(), &clone);
        assert_eq!(report.verdict, "detector_miss");
        assert!(report.prosody_cues.is_empty());
    }

    #[test]
    fn sad_gate_above_align_is_a_detector_miss() {
        let mut clone = published_clone("synthetic");
        clone.love_mean_cos = Some(0.30);
        clone.neutral_mean_cos = Some(0.10);
        clone.e2v_sad = Some(0.9);
        clone.decision = Some(0.0);
        let report = diagnose(&published_reference(), &clone);
        assert_eq!(report.verdict, "detector_miss");
        assert!(report.evidence.iter().any(|e| e == "sad_gate"));
    }

    #[test]
    fn single_f0_cue_with_collapsed_margin_is_mixed() {
        let mut clone = published_clone("synthetic");
        clone.love_mean_cos = Some(0.04);
        clone.neutral_mean_cos = Some(0.02);
        clone.prosody = Some(view(-8.0, -22.0, 2.4 - 1.2));
        let report = diagnose(&ref_obs(), &clone);
        assert_eq!(report.verdict, "mixed");
    }

    #[test]
    fn gate_math_matches_published_emotion_match() {
        let doc: Value = serde_json::from_str(include_str!("../fixtures/samantha_love_gate.json")).unwrap();
        for set in doc["gate_sets"].as_array().unwrap() {
            let rows: Vec<BlendRow> = serde_json::from_value(set["rows"].clone()).unwrap();
            let emb = set["emb_sim"].as_f64().unwrap();
            let got = emotion_match(&rows, emb).unwrap();
            let published = set["emotion_match"].as_f64().unwrap();
            assert!((got - published).abs() < 1e-12, "{} got {got} published {published}", set["id"]);
            let attr = attribute_love(&rows, emb, GATE).unwrap();
            assert!(!attr.pass);
            assert!(attr.pass_if_love_matched, "{}", set["id"]);
            assert!((attr.love_score - CLONE_LOVE_MATCH).abs() < 1e-12);
        }
        let report = diagnose_gate_document(&doc);
        let clones = report["clones"].as_array().unwrap();
        assert_eq!(clones.len(), 2);
        for c in clones {
            assert_eq!(c["verdict"], "insufficient_evidence");
        }
        assert_eq!(report["jarvis"]["gate_pass"], true);
        assert_eq!(report["jarvis"]["love_in_judged_set"], false);
        assert!((report["reference_margin"].as_f64().unwrap() - (REF_LOVE_MEAN_COS - REF_NEU_MEAN_COS)).abs() < 1e-15);
    }

    #[test]
    fn jarvis_numbers_clear_the_gate() {
        let doc: Value = serde_json::from_str(include_str!("../fixtures/samantha_love_gate.json")).unwrap();
        for set in doc["jarvis"]["sets"].as_array().unwrap() {
            let em = set["emotion_match"].as_f64().unwrap();
            assert!(em >= GATE, "{} {em}", set["id"]);
            let rows: Vec<BlendRow> = serde_json::from_value(set["rows"].clone()).unwrap();
            assert!(rows.iter().all(|r| r.category != "love" && r.category != "seductive"));
            let got = emotion_match(&rows, set["emb_sim"].as_f64().unwrap()).unwrap();
            assert!((got - em).abs() < 1e-12);
        }
    }
}
