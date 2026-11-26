// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One JSON report. Scores that were not measured are null. `claimed` is false.

use std::collections::BTreeMap;

use crate::gate::{self, Check, Rollout};
use crate::golden::{self, TurnReport};
use crate::jsonutil::{pretty, J};
use crate::roundtrip::RoundTrip;

pub struct Report {
    pub golden: Vec<TurnReport>,
    pub roundtrip: RoundTrip,
}

pub fn build(golden: Vec<TurnReport>, roundtrip: RoundTrip) -> Report {
    Report { golden, roundtrip }
}

/// Report label. `GALION_HORBM_DEV_PERSONA` when it is non-empty, otherwise `configured`.
pub fn persona_label(dev_persona: Option<&str>) -> String {
    match dev_persona.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None => "configured".to_string(),
    }
}

pub fn to_json(report: &Report) -> String {
    let label = persona_label(std::env::var("GALION_HORBM_DEV_PERSONA").ok().as_deref());
    pretty(&J::obj(vec![
        ("schema", J::str("galion-eval-golden/1")),
        ("persona", J::str(label)),
        ("claimed", J::Bool(false)),
        ("scores_fabricated", J::Bool(false)),
        ("audio_committed", J::Bool(false)),
        ("rollout_gate", gate_block()),
        ("stream_bar_ms", J::Num(gate::STREAM_BAR_MS)),
        (
            "golden",
            J::Arr(report.golden.iter().map(turn_json).collect()),
        ),
        ("golden_pass", J::Bool(golden::all_passed(&report.golden))),
        ("roundtrip", trip_json(&report.roundtrip)),
    ]))
}

fn gate_block() -> J {
    J::obj(vec![
        ("wer_max", J::Num(gate::WER_MAX)),
        ("wer_comparison", J::str("<=")),
        ("ecapa_min_exclusive", J::Num(gate::ECAPA_MIN)),
        ("ecapa_comparison", J::str(">")),
        (
            "ecapa_note",
            J::str("Strict greater-than, matching tools/emotion-v2/harness/metrics.py ecapa_passes. A cosine of exactly 0.7057 fails. The swarm brief wrote >=; this crate follows the locked comparison."),
        ),
        ("emotion_min", J::Num(gate::EMOTION_MIN)),
        ("emotion_comparison", J::str(">=")),
        ("emotion_formula", J::str(gate::EMOTION_FORMULA)),
        (
            "source",
            J::str("tools/emotion-v2/thresholds.toml, tools/emotion-v2/harness/metrics.py, tools/symphony/listen-back ROLLOUT_* constants"),
        ),
    ])
}

fn turn_json(turn: &TurnReport) -> J {
    J::obj(vec![
        ("script", J::str(&turn.script)),
        ("kind", J::str(&turn.kind)),
        ("user", J::str(&turn.user)),
        ("reply", J::str(&turn.reply)),
        ("intent", J::str(&turn.intent)),
        ("arm", J::str(&turn.arm)),
        ("verdict", J::str(turn.verdict)),
        (
            "hits",
            J::Arr(
                turn.hits
                    .iter()
                    .map(|h| {
                        J::obj(vec![
                            ("rule", J::str(h.rule)),
                            ("detail", J::str(&h.detail)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "notes",
            J::Arr(turn.notes.iter().map(J::str).collect()),
        ),
        ("mood_arousal", J::Num(f64::from(turn.arousal))),
        ("mood_valence", J::Num(f64::from(turn.valence))),
        ("mood_dominance", J::Num(f64::from(turn.dominance))),
    ])
}

fn trip_json(trip: &RoundTrip) -> J {
    J::obj(vec![
        ("voice_note", J::str(&trip.voice_note)),
        ("piper", opt_str(trip.piper.as_deref())),
        ("model", opt_str(trip.model.as_deref())),
        ("sample_rate", opt_num(trip.sample_rate.map(|n| n as f64))),
        ("model_load_ms", opt_num(trip.model_load_ms)),
        (
            "utterances",
            J::Arr(trip.utterances.iter().map(utt_json).collect()),
        ),
        ("wer", check_json(&trip.wer)),
        ("ecapa", check_json(&trip.ecapa)),
        ("emotion_match", check_json(&trip.emotion)),
        ("rollout", rollout_json(&trip.rollout)),
        ("tts_first_audio", check_json(&trip.tts_first_audio)),
        ("asr_first_partial", check_json(&trip.asr_first_partial)),
        (
            "notes",
            J::Arr(trip.notes.iter().map(J::str).collect()),
        ),
    ])
}

fn utt_json(utt: &crate::roundtrip::Utterance) -> J {
    J::obj(vec![
        ("reference", J::str(&utt.reference)),
        ("hypothesis", J::str(&utt.hypothesis)),
        ("wer", opt_num(utt.wer)),
        ("pcm_bytes", J::Int(utt.pcm_bytes as i64)),
        ("audio_s", opt_num(utt.audio_s)),
        ("first_audio_ms", opt_num(utt.first_audio_ms)),
        ("tts_wall_ms", J::Num(utt.tts_wall_ms)),
        ("asr_wall_ms", opt_num(utt.asr_wall_ms)),
        ("asr_first_partial_ms", opt_num(utt.asr_first_partial_ms)),
    ])
}

fn check_json(check: &Check) -> J {
    J::obj(vec![
        ("name", J::str(&check.name)),
        ("status", J::str(check.status.label())),
        ("comparison", J::str(&check.comparison)),
        ("threshold", opt_num(check.threshold)),
        ("value", opt_num(check.value)),
        ("detail", J::str(&check.detail)),
    ])
}

fn rollout_json(roll: &Rollout) -> J {
    let mut map = BTreeMap::new();
    map.insert("status".into(), J::str(roll.status.label()));
    map.insert(
        "pass".into(),
        match roll.pass {
            Some(true) => J::Bool(true),
            Some(false) => J::Bool(false),
            None => J::Null,
        },
    );
    map.insert("claimed".into(), J::Bool(roll.claimed));
    map.insert("reason".into(), J::str(&roll.reason));
    J::Obj(map)
}

fn opt_str(value: Option<&str>) -> J {
    match value {
        Some(s) => J::str(s),
        None => J::Null,
    }
}

fn opt_num(value: Option<f64>) -> J {
    match value {
        Some(n) if n.is_finite() => J::Num(n),
        _ => J::Null,
    }
}

pub fn summary(report: &Report) -> String {
    let mut lines = Vec::new();
    lines.push("configured persona eval-golden. claimed=false. No audio in this report.".into());
    for turn in &report.golden {
        let extra = if turn.hits.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                turn.hits
                    .iter()
                    .map(|h| h.rule)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        lines.push(format!(
            "golden {}/{}: {}{extra}",
            turn.script, turn.kind, turn.verdict
        ));
    }
    let trip = &report.roundtrip;
    lines.push(format!(
        "wer: {} {}",
        trip.wer.status.label(),
        fmt_value(trip.wer.value)
    ));
    lines.push(format!(
        "ecapa: {} {}",
        trip.ecapa.status.label(),
        fmt_value(trip.ecapa.value)
    ));
    lines.push(format!(
        "emotion_match: {} {}",
        trip.emotion.status.label(),
        fmt_value(trip.emotion.value)
    ));
    lines.push(format!(
        "rollout: {} pass={}",
        trip.rollout.status.label(),
        fmt_pass(trip.rollout.pass)
    ));
    lines.push(format!(
        "tts_first_audio_ms: {} {}",
        trip.tts_first_audio.status.label(),
        fmt_value(trip.tts_first_audio.value)
    ));
    lines.push(format!(
        "asr_first_partial_ms: {} {}",
        trip.asr_first_partial.status.label(),
        fmt_value(trip.asr_first_partial.value)
    ));
    if let Some(ms) = trip.model_load_ms {
        lines.push(format!("piper_model_load_ms: {ms:.1}"));
    }
    for utt in &trip.utterances {
        lines.push(format!(
            "utt wer={} first_audio_ms={} asr_ms={} hyp=\"{}\" ref=\"{}\"",
            fmt_value(utt.wer),
            fmt_value(utt.first_audio_ms),
            fmt_value(utt.asr_wall_ms),
            utt.hypothesis,
            utt.reference
        ));
    }
    lines.join("\n")
}

fn fmt_value(value: Option<f64>) -> String {
    match value {
        Some(n) if n.is_finite() => format!("{n:.4}"),
        _ => "null".into(),
    }
}

fn fmt_pass(pass: Option<bool>) -> &'static str {
    match pass {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::FakeReplyEngine;
    use crate::gate::{self, Status};
    use crate::golden::run_golden;

    #[test]
    fn summary_uses_the_pass_fail_unmeasured_words() {
        let mut engine = FakeReplyEngine::new();
        let golden = run_golden(&mut engine);
        let trip = crate::roundtrip::RoundTrip {
            voice_note: "none".into(),
            piper: None,
            model: None,
            sample_rate: None,
            model_load_ms: None,
            utterances: Vec::new(),
            wer: gate::Check::unmeasured("wer", "<=", Some(gate::WER_MAX), "absent"),
            ecapa: gate::Check::unmeasured("ecapa", ">", Some(gate::ECAPA_MIN), "absent"),
            emotion: gate::Check::unmeasured(
                "emotion_match",
                ">=",
                Some(gate::EMOTION_MIN),
                "absent",
            ),
            rollout: gate::judge_rollout(
                &gate::Check::unmeasured("wer", "<=", Some(gate::WER_MAX), "absent"),
                &gate::Check::unmeasured("ecapa", ">", Some(gate::ECAPA_MIN), "absent"),
                &gate::Check::unmeasured("emotion_match", ">=", Some(gate::EMOTION_MIN), "absent"),
            ),
            tts_first_audio: gate::Check::unmeasured(
                "tts_first_audio_ms",
                "<=",
                Some(80.0),
                "absent",
            ),
            asr_first_partial: gate::Check::unmeasured(
                "asr_first_partial_ms",
                "<=",
                Some(80.0),
                "absent",
            ),
            notes: Vec::new(),
        };
        assert_eq!(trip.rollout.status, Status::Unmeasured);
        let report = build(golden, trip);
        let text = summary(&report);
        assert!(text.contains("PASS"));
        assert!(text.contains("UNMEASURED"));
        let json = to_json(&report);
        assert!(json.contains("\"claimed\": false"));
        assert!(json.contains("0.7057"));
        assert!(json.contains("0.83"));
        assert!(json.contains("0.018"));
        assert!(!json.contains("pcm_b64"));
    }

    #[test]
    fn report_label_is_the_configured_id() {
        assert_eq!(persona_label(None), "configured");
        assert_eq!(persona_label(Some("")), "configured");
        assert_eq!(persona_label(Some("  ")), "configured");
        assert_eq!(persona_label(Some("northwind")), "northwind");
    }
}
