// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic signals only. No fixture audio is read from disk.

use galion_articulation::acoustic;
use galion_listen_back::contract::{self, resolve};
use galion_listen_back::decode::{self, ReadBack};
use galion_listen_back::fixture::BreathSpan;
use galion_listen_back::render;
use galion_listen_back::report;
use galion_listen_back::score::{self, speaker_kept};
use galion_listen_back::{
    run_all, Options, ROLLOUT_ECAPA_MIN, ROLLOUT_EMOTION_MIN, ROLLOUT_WER_MAX,
};
use std::sync::OnceLock;

fn vowel(f0: f64, f1: f64, f2: f64) -> galion_articulation::Articulation {
    let mut art = acoustic::analyse(&synth(f0, f1, f2), render::SR).0;
    art.finish();
    art
}

fn synth(f0: f64, f1: f64, f2: f64) -> Vec<f64> {
    let sr = render::SR;
    let n = (0.80 * sr) as usize;
    let mut y = vec![0.0; n];
    let period = (sr / f0).round().max(1.0) as usize;
    for i in (0..n).step_by(period) {
        y[i] = 1.0;
    }
    for (f, bw) in [(f1, 80.0), (f2, 120.0)] {
        let r = (-std::f64::consts::PI * bw / sr).exp();
        let c = 2.0 * r * (2.0 * std::f64::consts::PI * f / sr).cos();
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in y.iter_mut() {
            let o = *v + c * y1 - r * r * y2;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let m = y.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    y.iter().map(|v| 0.5 * v / m).collect()
}

fn shared() -> &'static galion_listen_back::Report {
    static R: OnceLock<galion_listen_back::Report> = OnceLock::new();
    R.get_or_init(|| run_all(&Options::synthetic()).expect("loop"))
}

fn scenario(id: &str) -> &'static galion_listen_back::report::ScenarioReport {
    shared().scenarios.iter().find(|s| s.id == id).expect(id)
}

#[test]
fn names_follow_the_shared_inventory() {
    assert_eq!(resolve("embarrassment").unwrap().category, "embarrassment");
    assert_eq!(resolve("embarrassment").unwrap().track, "prosody");
    assert_eq!(
        contract::prosody_tier(resolve("embarrassment").unwrap()),
        "extended"
    );
    assert_eq!(
        resolve("laughter").unwrap(),
        contract::Resolved {
            track: "burst",
            category: "amusement"
        }
    );
    assert_eq!(resolve("seductive").unwrap().category, "desire");
    assert_eq!(resolve("neutral").unwrap().track, "baseline");
    assert!(resolve("breath").unwrap_err().contains("physiology"));
    assert!(resolve("sigh").is_err());
    assert!(resolve("happy").is_err());
    assert!(resolve("surprised").is_err());
    let eval = include_str!("../../articulation/evals/config.toml");
    for name in contract::COWEN {
        assert!(
            eval.contains(&format!("\"{name}\"")),
            "{name} missing from the eval inventory"
        );
    }
    assert!(eval.contains("0.7057"));
    assert!(eval.contains("guilt"));
}

#[test]
fn breath_matcher_counts_partial_plans() {
    let intended = [
        BreathSpan {
            start_s: 0.0,
            dur_s: 0.16,
        },
        BreathSpan {
            start_s: 0.96,
            dur_s: 0.16,
        },
    ];
    let one = [BreathSpan {
        start_s: 0.0,
        dur_s: 0.16,
    }];
    assert_eq!(score::breath_plan_match(&intended, &one), 0.5);
    assert_eq!(score::breath_plan_match(&intended, &intended), 1.0);
    assert_eq!(score::breath_plan_match(&[], &[]), 1.0);
    assert_eq!(score::breath_plan_match(&one, &[]), 0.0);
}

#[test]
fn lungs_plan_inhales_are_read_not_replanned() {
    let plan = serde_json::json!({
        "schema": "galion-lungs/1",
        "not_emotion": true,
        "segments": [
            {"kind": "inhale", "start_s": 0.0, "end_s": 0.24},
            {"kind": "speech", "start_s": 0.24, "end_s": 1.0}
        ]
    });
    let got = score::lungs_inhales(&plan).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].dur_s, 0.24);
    assert!(score::lungs_inhales(&serde_json::json!({"schema": "other"})).is_err());
}

#[test]
fn decode_profile_supplies_the_feeling_label() {
    let intent = contract::from_fixture(
        galion_listen_back::fixture::builtin()
            .iter()
            .find(|f| f.id == "embarrassment-word")
            .unwrap(),
    )
    .unwrap();
    let samples = render::reference(&intent);
    let mut read: ReadBack = decode::analyse(&samples, render::SR).unwrap();
    assert!(read.feeling_top.is_none());
    decode::merge_profile(
        &mut read,
        &serde_json::json!({
            "schema": "galion-voice-decode/2",
            "speaker": {"f0_hz": 180.0},
            "measured": {
                "track": "prosody",
                "top": "embarrassment",
                "ticks": [{"t_ms": 0, "arousal": 0.0, "valence": 0.0, "dominance": 0.0, "intensity": 0.4}]
            }
        }),
    )
    .unwrap();
    let (_score, view) = score::feeling(&intent, &read);
    assert!(view.category_compared);
    assert_eq!(view.readback_category.as_deref(), Some("embarrassment"));
    assert_eq!(read.profile_speaker_f0_hz, Some(180.0));
    assert!(decode::merge_profile(&mut read, &serde_json::json!({"schema": "nope"})).is_err());
}

#[test]
fn same_tract_stays_above_the_other_speaker() {
    let base = vowel(180.0, 700.0, 1200.0);
    let same = vowel(182.0, 710.0, 1220.0);
    let other = vowel(110.0, 400.0, 2200.0);
    let kept = speaker_kept(&base, &same);
    let moved = speaker_kept(&base, &other);
    assert!(kept > 0.85, "same speaker {kept}");
    assert!(moved < 0.55, "other speaker {moved}");
    let tract_same = galion_articulation::score::compare(&base, &same).total;
    let tract_other = galion_articulation::score::compare(&base, &other).total;
    assert!(
        tract_same > tract_other,
        "articulation same {tract_same} other {tract_other}"
    );
}

#[test]
fn closed_loop_corrects_once_and_records_the_gate() {
    let first = shared();
    let again = run_all(&Options::synthetic()).unwrap();
    assert_eq!(report::to_json(first), report::to_json(&again));

    assert_eq!(first.schema, "galion-listen-back/1");
    assert_eq!(first.correction_passes, 1);
    assert!(!first.rollout_gate.measured);
    assert_eq!(first.rollout_gate.wer_max, ROLLOUT_WER_MAX);
    assert_eq!(first.rollout_gate.ecapa_min, ROLLOUT_ECAPA_MIN);
    assert_eq!(first.rollout_gate.emotion_min, ROLLOUT_EMOTION_MIN);
    assert!((ROLLOUT_WER_MAX - 0.018).abs() < 1e-12);
    assert!((ROLLOUT_ECAPA_MIN - 0.7057).abs() < 1e-12);
    assert!((ROLLOUT_EMOTION_MIN - 0.83).abs() < 1e-12);

    let text = report::summary(first);
    assert!(text.contains("not measured"));
    assert!(text.contains("speaker f0 steered: false"));
    assert!(text.contains("embarrassment-word"));
    assert!(first.stubs.iter().any(|s| s.contains("TODO(S1")));
    assert!(first.stubs.iter().any(|s| s.contains("TODO(S6")));
    assert!(first.stubs.iter().any(|s| s.contains("PR #191")));
    assert!(first.stubs.iter().any(|s| s.contains("PR #195")));

    for s in &first.scenarios {
        assert_eq!(s.passes.len(), 2, "{}", s.id);
        assert!(!s.correction.speaker_f0_steered, "{}", s.id);
        assert_eq!(s.passes[0].baseline.f0_hz, 180.0);
        assert_eq!(s.passes[1].baseline.f0_hz, 180.0);
        assert!(!s.passes[0].baseline.steered && !s.passes[1].baseline.steered);
        assert_eq!(s.passes[1].applied_pitch_st, s.correction.to_pitch_st);
        assert_eq!(s.passes[0].body.nose_status, "stub");
        assert!(s.intent_source.contains("stub"));
        assert!(s.lungs.status == "stub");
    }

    let neutral = scenario("neutral-hold");
    assert_eq!(neutral.track, "baseline");
    assert_eq!(neutral.correction.pitch_st_delta, 0.0);
    assert_eq!(neutral.correction.breaths_inserted, 0);
    assert!(neutral.passes[0].scores.breath_plan_match > 0.99);
    assert!(neutral.passes[1].scores.breath_plan_match > 0.99);
    assert!(
        neutral.passes[0].scores.speaker_kept > 0.85,
        "{:?}",
        neutral.passes[0].scores
    );
    assert!(
        (neutral.passes[0].scores.pitch_voice_quality
            - neutral.passes[1].scores.pitch_voice_quality)
            .abs()
            < 0.02
    );

    let sadness = scenario("sadness-fall");
    assert_eq!(sadness.prosody_tier, "reliable");
    let e1 = sadness.passes[0].pitch_error_st.expect("pitch");
    let e2 = sadness.passes[1].pitch_error_st.expect("pitch");
    assert!(e2.abs() + 0.15 < e1.abs(), "sadness pitch {e1} -> {e2}");
    assert!(
        sadness.passes[1].scores.breath_plan_match > sadness.passes[0].scores.breath_plan_match
    );
    assert!(
        sadness.passes[1].scores.feeling_match + 0.02 >= sadness.passes[0].scores.feeling_match
    );
    assert!(
        sadness.passes[0].scores.speaker_kept > 0.7,
        "speaker {}",
        sadness.passes[0].scores.speaker_kept
    );
    assert!(
        (sadness.passes[0].applied_pitch_st - render::STANDIN_PITCH_FRACTION * -2.0).abs() < 1e-3
    );

    let shy = scenario("embarrassment-word");
    assert_eq!(shy.category, "embarrassment");
    assert_eq!(shy.prosody_tier, "extended");
    assert_eq!(shy.track, "prosody");
    let a = shy.passes[0].pitch_error_st.expect("pitch");
    let b = shy.passes[1].pitch_error_st.expect("pitch");
    assert!(b.abs() + 0.15 < a.abs(), "embarrassment pitch {a} -> {b}");

    let laugh = scenario("laughter-burst");
    assert_eq!(laugh.track, "burst");
    assert_eq!(laugh.category, "amusement");
    assert_eq!(laugh.correction.pitch_st_delta, 0.0);
    assert_eq!(laugh.correction.breaths_inserted, 1);
    assert!(
        laugh.passes[1].scores.breath_plan_match > laugh.passes[0].scores.breath_plan_match + 0.5
    );
}
