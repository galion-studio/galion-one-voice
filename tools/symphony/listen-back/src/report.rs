// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Deterministic report. JSON for the integrator, and a short text summary.
//! No timestamps, no sample arrays, no measured rollout-gate numbers.

use serde::{Deserialize, Serialize};

use crate::fixture::BreathSpan;
use crate::score::{BaselineView, BodyView, FeelingView, LayerScores};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RolloutGate {
    pub wer_max: f64,
    pub ecapa_min: f64,
    pub emotion_min: f64,
    pub measured: bool,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Correction {
    pub from_pitch_st: f64,
    pub to_pitch_st: f64,
    pub pitch_st_delta: f64,
    pub place_breaths: bool,
    pub breaths_inserted: usize,
    /// The baseline F0 field is not an output of the correction.
    pub speaker_f0_steered: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LungsNote {
    pub status: String,
    pub inhales: Vec<BreathSpan>,
    pub agreement: Option<f64>,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PassReport {
    pub index: u32,
    pub renderer: String,
    pub decoder: String,
    pub applied_pitch_st: f64,
    pub place_breaths: bool,
    pub pitch_error_st: Option<f64>,
    pub scores: LayerScores,
    pub feeling: FeelingView,
    pub body: BodyView,
    pub baseline: BaselineView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScenarioReport {
    pub id: String,
    pub text: String,
    pub track: String,
    pub category: String,
    pub prosody_tier: String,
    pub intent_source: String,
    pub feeling_source: String,
    pub lungs: LungsNote,
    pub correction: Correction,
    pub passes: Vec<PassReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub schema: String,
    /// Always one. The second pass is the re-score, not a second correction.
    pub correction_passes: u32,
    pub rollout_gate: RolloutGate,
    pub scenarios: Vec<ScenarioReport>,
    pub stubs: Vec<String>,
}

pub fn to_json(report: &Report) -> String {
    let mut body = serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into());
    body.push('\n');
    body
}

pub fn summary(report: &Report) -> String {
    let mut lines = Vec::new();
    lines.push("galion-listen-back/1".into());
    lines.push("One correction pass, then a second score.".into());
    lines.push(
        "Rollout gate recorded, not measured: WER <= 1.8%, ECAPA > 0.7057, emotion >= 0.83.".into(),
    );
    lines.push(String::new());
    for s in &report.scenarios {
        lines.push(format!(
            "{}  {} {}  tier {}",
            s.id, s.track, s.category, s.prosody_tier
        ));
        for p in &s.passes {
            let err = p
                .pitch_error_st
                .map(|e| format!("{e:.4}"))
                .unwrap_or_else(|| "none".into());
            lines.push(format!(
                "  pass {}  feeling {:.4}  breath {:.4}  pitch {:.4}  articulation {:.4}  speaker {:.4}  pitch_error_st {}",
                p.index,
                p.scores.feeling_match,
                p.scores.breath_plan_match,
                p.scores.pitch_voice_quality,
                p.scores.articulation,
                p.scores.speaker_kept,
                err
            ));
            if p.index == 1 {
                lines.push(format!(
                    "  correction  pitch_st {:.4} -> {:.4}  breaths placed {}  breaths inserted {}  speaker f0 steered: {}",
                    s.correction.from_pitch_st,
                    s.correction.to_pitch_st,
                    s.correction.place_breaths,
                    s.correction.breaths_inserted,
                    s.correction.speaker_f0_steered
                ));
            }
        }
        lines.push(String::new());
    }
    lines.push("stubs".into());
    for stub in &report.stubs {
        lines.push(format!("- {stub}"));
    }
    lines.push(String::new());
    lines.join("\n")
}
