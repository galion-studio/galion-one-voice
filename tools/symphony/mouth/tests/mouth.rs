// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic tests. No audio, no model, no measurement.

use std::fs;
use std::path::PathBuf;

use galion_mouth::{
    listen, parse_feelings, parse_scales, plan, read_json, read_via_binary, IntentStub, MouthError,
    Scales,
};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn scales(text: &str) -> Scales {
    parse_scales(text).unwrap()
}

fn intent(text: &str) -> IntentStub {
    serde_json::from_str(text).unwrap()
}

fn feelings(text: &str) -> Vec<galion_mouth::FeelingTick> {
    parse_feelings(&serde_json::from_str(text).unwrap()).unwrap()
}

const GRID: &str = r#"
[timing]
phone_ms = 80
"#;

fn with_scales(body: &str) -> Scales {
    scales(&format!("{GRID}\n{body}"))
}

fn one(intent_json: &str, feeling_json: &str, scale_body: &str) -> galion_mouth::MouthPlan {
    plan(
        &intent(intent_json),
        &feelings(feeling_json),
        &with_scales(scale_body),
    )
    .unwrap()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn inventory_matches_the_emotion_file() {
    galion_mouth::inventory::check_inventory_file(&repo("../../../configs/emotions-v2.toml"))
        .unwrap();
}

#[test]
fn shipped_scales_are_unknown() {
    let shipped = galion_mouth::load_scales(&repo("../../../configs/symphony/mouth.toml")).unwrap();
    assert_eq!(shipped.phone_ms, 80);
    for key in [
        "f1_hz",
        "f2_hz",
        "f3_hz",
        "smile_formant_hz",
        "smile_spread",
        "address_f1_hz",
        "address_jaw",
        "address_lip_opening",
        "jaw_emotion",
        "rate",
        "precision",
        "coarticulation",
    ] {
        assert!(shipped.get(None, key).is_none(), "{key}");
        assert!(shipped.get(Some("hot_anger"), key).is_none(), "{key}");
    }
    let plan = plan(
        &intent(r#"{"text":"no","phones":[{"symbol":"aa"}]}"#),
        &feelings(r#"{"emotion":"anger","intensity":1,"dominance":0.4}"#),
        &shipped,
    )
    .unwrap();
    let tick = &plan.ticks[0];
    assert_eq!(tick.f1.direction, "higher");
    assert!(tick.f1.amount.is_none());
    assert_eq!(tick.f2.direction, "not_applied");
    assert_eq!(tick.jaw_emotion.direction, "more_open");
    assert!(tick.jaw_emotion.amount.is_none());
    assert_eq!(tick.rate.direction, "faster");
    assert!(tick.rate.amount.is_none());
    assert_eq!(plan.clone_mouth.speaking_rate.amount, None);
    assert_eq!(
        plan.unknown,
        vec![
            "coarticulation".to_string(),
            "f1_hz".into(),
            "jaw_emotion".into(),
            "rate".into(),
        ]
    );
    assert!(plan.intent.todo.contains("galion-mne-contract"));
    assert!(plan.clone_mouth.integrator.contains("194"));
    assert_eq!(plan.tick_ms, 80);
}

#[test]
fn missing_rows_stay_missing() {
    let plan = one(
        r#"{"phones":[{"symbol":"iy"}]}"#,
        r#"{"emotion":"embarrassment","intensity":1}"#,
        "",
    );
    let tick = &plan.ticks[0];
    assert_eq!(tick.f1.direction, "not_applied");
    assert_eq!(tick.rate.direction, "not_applied");
    assert_eq!(
        tick.prosody_tier.as_deref(),
        Some("valid in prosody, lower recognition evidence")
    );
    assert!(!plan.unknown.iter().any(|k| k == "f1_hz"));

    let amusement = one(
        r#"{"phones":[{"symbol":"iy"}]}"#,
        r#"{"emotion":"amusement","intensity":1}"#,
        "",
    );
    assert_eq!(amusement.ticks[0].f1.direction, "not_applied");
    assert_eq!(amusement.ticks[0].rate.direction, "not_applied");
    assert!(amusement.ticks[0].withheld_f1.is_none());

    let desire = one(
        r#"{"phones":[{"symbol":"iy"}]}"#,
        r#"{"emotion":"seductive","intensity":1}"#,
        "",
    );
    assert_eq!(desire.ticks[0].feeling, "desire");
    assert_eq!(desire.ticks[0].alias_from.as_deref(), Some("seductive"));
    assert_eq!(desire.ticks[0].f1.direction, "not_applied");
    assert_eq!(desire.ticks[0].rate.direction, "not_applied");
}

#[test]
fn burst_does_not_apply_speech_signs() {
    let laugh = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"laughter","intensity":1}"#,
        "",
    );
    assert_eq!(laugh.ticks[0].track, "burst");
    assert_eq!(laugh.ticks[0].feeling, "amusement");
    assert_eq!(laugh.ticks[0].alias_from.as_deref(), Some("laughter"));
    assert_eq!(laugh.ticks[0].f1.direction, "not_applied");
    assert!(laugh.ticks[0].withheld_f1.is_none());

    let burst_anger = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","track":"burst","intensity":1}"#,
        "",
    );
    assert_eq!(burst_anger.ticks[0].f1.direction, "not_applied");
    assert_eq!(burst_anger.ticks[0].rate.direction, "not_applied");
    assert_eq!(burst_anger.ticks[0].withheld_f1.as_deref(), Some("higher"));
    assert_eq!(
        burst_anger.ticks[0].withheld_rate.as_deref(),
        Some("faster")
    );
    assert!(burst_anger.clone_mouth.speaking_rate.amount.is_none());
}

#[test]
fn decoder_burst_array_is_the_burst_track() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"blend":[{"id":"neutral","w":1}],"burst":[{"id":"amusement","w":1}],"intensity":1}"#,
        "",
    );
    assert_eq!(plan.ticks[0].track, "burst");
    assert_eq!(plan.ticks[0].feeling, "amusement");
    assert_eq!(plan.ticks[0].f1.direction, "not_applied");
}

#[test]
fn splits_and_retired_names_are_errors() {
    let happy = parse_feelings(&serde_json::json!({"emotion": "happy", "intensity": 1.0}));
    match happy {
        Err(MouthError::Split { options, .. }) => {
            assert_eq!(options, vec!["amusement".to_string(), "interest".into()]);
        }
        other => panic!("{other:?}"),
    }
    let sigh = parse_feelings(&serde_json::json!({"emotion": "sigh", "intensity": 1.0}));
    assert!(matches!(sigh, Err(MouthError::BurstDetector { .. })));
    let guilt = parse_feelings(&serde_json::json!({"emotion": "guilt", "intensity": 1.0}));
    assert!(matches!(guilt, Err(MouthError::NotACategory(_))));
}

#[test]
fn neutral_and_zero_intensity_are_baseline() {
    let calm = one(
        r#"{"phones":[{"symbol":"iy"}]}"#,
        r#"{"emotion":"calm","intensity":1}"#,
        "",
    );
    assert_eq!(calm.ticks[0].track, "baseline");
    assert_eq!(calm.ticks[0].feeling, "neutral");
    assert_eq!(calm.ticks[0].f1.direction, "not_applied");

    let dropped = one(
        r#"{"phones":[{"symbol":"iy"}]}"#,
        r#"{"emotion":"anger"}"#,
        "",
    );
    assert_eq!(dropped.ticks[0].feeling, "neutral");
}

#[test]
fn valence_does_not_spread_the_lips_and_arousal_does_not_scale() {
    let smile_off = one(
        r#"{"phones":[{"symbol":"iy"}],"smile":false}"#,
        r#"{"emotion":"contentment","intensity":1,"valence":1,"arousal":1}"#,
        "",
    );
    assert_eq!(smile_off.ticks[0].lip_intent.direction, "not_applied");
    assert_eq!(smile_off.ticks[0].lips, "spread");

    let low = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1,"arousal":0}"#,
        "[scales]\nf1_hz = 10\n",
    );
    let high = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1,"arousal":1}"#,
        "[scales]\nf1_hz = 10\n",
    );
    close(
        low.ticks[0].f1.amount.unwrap(),
        high.ticks[0].f1.amount.unwrap(),
    );
}

#[test]
fn smile_raises_formants_and_a_known_step_sums() {
    let unknown = one(
        r#"{"phones":[{"symbol":"uw"}],"smile":true}"#,
        r#"{"emotion":"neutral","intensity":1}"#,
        "",
    );
    assert_eq!(unknown.ticks[0].lips, "round");
    assert_eq!(unknown.ticks[0].lip_intent.direction, "spread");
    assert!(unknown.ticks[0].lip_intent.amount.is_none());
    assert_eq!(unknown.ticks[0].f1.direction, "higher");
    assert_eq!(unknown.ticks[0].f2.direction, "higher");
    assert_eq!(unknown.ticks[0].f3.direction, "higher");
    assert!(unknown.ticks[0].f1.amount.is_none());
    assert!(unknown.unknown.iter().any(|k| k == "smile_formant_hz"));

    let known = one(
        r#"{"phones":[{"symbol":"iy"}],"smile":true}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\nf1_hz = 10\nsmile_formant_hz = 4\nrate = 0.25\n",
    );
    close(known.ticks[0].f1.amount.unwrap(), 14.0);
    close(known.ticks[0].f2.amount.unwrap(), 4.0);
    close(known.ticks[0].f3.amount.unwrap(), 4.0);
    close(known.ticks[0].rate.amount.unwrap(), 1.25);
    assert_eq!(known.ticks[0].lips, "spread");
    let f1_names: Vec<_> = known.ticks[0]
        .f1
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(f1_names, vec!["f1_hz", "smile_formant_hz"]);
}

#[test]
fn unknown_feeling_step_blocks_a_partial_sum() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa"}],"smile":true}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\nsmile_formant_hz = 4\n",
    );
    assert!(plan.ticks[0].f1.amount.is_none());
    assert_eq!(plan.ticks[0].f1.direction, "higher");
    let smile = plan.ticks[0]
        .f1
        .components
        .iter()
        .find(|c| c.name == "smile_formant_hz")
        .unwrap();
    close(smile.amount.unwrap(), 4.0);
    assert!(!smile.blocked);
    let anger = plan.ticks[0]
        .f1
        .components
        .iter()
        .find(|c| c.name == "f1_hz")
        .unwrap();
    assert!(anger.blocked);
    assert!(anger.amount.is_none());
}

#[test]
fn fear_and_sadness_and_adoration_and_elation() {
    for (emotion, dominance, pole) in [
        ("fear", 0.5, "panic"),
        ("fearful", -0.5, "anxiety"),
        ("fear", 0.0, "fear_modal"),
    ] {
        let plan = one(
            r#"{"phones":[{"symbol":"aa"}]}"#,
            &format!(r#"{{"emotion":"{emotion}","intensity":1,"dominance":{dominance}}}"#),
            "",
        );
        assert_eq!(plan.ticks[0].f1.direction, "lower", "{emotion} {dominance}");
        assert_eq!(plan.ticks[0].rate.direction, "faster");
        assert_eq!(plan.ticks[0].pole.as_deref(), Some(pole));
        assert!(plan.ticks[0].f1.amount.is_none());
    }
    let sad = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"sad","intensity":1}"#,
        "",
    );
    assert_eq!(sad.ticks[0].feeling, "sadness");
    assert_eq!(sad.ticks[0].f1.direction, "lower");
    assert_eq!(sad.ticks[0].jaw_emotion.direction, "more_closed");
    assert_eq!(sad.ticks[0].rate.direction, "slower");

    let tender = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"tender","intensity":1}"#,
        "",
    );
    assert_eq!(tender.ticks[0].feeling, "adoration");
    assert_eq!(tender.ticks[0].f1.direction, "not_applied");
    assert_eq!(tender.ticks[0].rate.direction, "slower");

    let elation = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"elation","intensity":1}"#,
        "",
    );
    assert_eq!(elation.ticks[0].rate.direction, "slower");
    assert_eq!(elation.ticks[0].f1.direction, "not_applied");
    assert_eq!(
        elation.ticks[0].prosody_tier.as_deref(),
        Some("valid in prosody, lower recognition evidence")
    );
}

#[test]
fn address_in_noise_and_cancellation() {
    let open = one(
        r#"{"phones":[{"symbol":"iy"}],"address_in_noise":true}"#,
        r#"{"emotion":"neutral","intensity":1}"#,
        "",
    );
    assert_eq!(open.ticks[0].f1.direction, "higher");
    assert!(open.ticks[0].f1.amount.is_none());
    assert_eq!(open.ticks[0].jaw_emotion.direction, "more_open");
    assert_eq!(open.ticks[0].lip_opening.direction, "more_open");
    assert_eq!(open.ticks[0].f2.direction, "not_applied");

    let cancel = one(
        r#"{"phones":[{"symbol":"aa"}],"address_in_noise":true}"#,
        r#"{"emotion":"fear","intensity":1,"dominance":0.2}"#,
        "[scales]\nf1_hz = 10\naddress_f1_hz = 10\njaw_emotion = 4\naddress_jaw = 4\n",
    );
    assert_eq!(cancel.ticks[0].f1.direction, "cancelled");
    close(cancel.ticks[0].f1.amount.unwrap(), 0.0);
    assert_eq!(cancel.ticks[0].jaw_emotion.direction, "cancelled");
    close(cancel.ticks[0].jaw_emotion.amount.unwrap(), 0.0);
}

#[test]
fn unresolved_when_sizes_disagree_and_one_is_unknown() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa"}],"address_in_noise":true}"#,
        r#"{"emotion":"fear","intensity":1}"#,
        "[scales]\naddress_f1_hz = 10\n",
    );
    assert_eq!(plan.ticks[0].f1.direction, "unresolved");
    assert!(plan.ticks[0].f1.amount.is_none());
}

#[test]
fn pole_step_is_config_not_a_different_sign() {
    let body = r#"
[scales]
rate = 0.1
[poles.hot_anger]
rate = 0.2
[poles.cold_anger]
rate = 0.05
"#;
    let hot = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1,"dominance":0.5}"#,
        body,
    );
    let cold = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1,"dominance":-0.5}"#,
        body,
    );
    let modal = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1,"dominance":0}"#,
        body,
    );
    assert_eq!(hot.ticks[0].f1.direction, "higher");
    assert_eq!(cold.ticks[0].f1.direction, "higher");
    close(hot.ticks[0].rate.amount.unwrap(), 1.2);
    close(cold.ticks[0].rate.amount.unwrap(), 1.05);
    close(modal.ticks[0].rate.amount.unwrap(), 1.1);
}

#[test]
fn blend_weight_is_the_contribution_and_a_mixture_can_cancel() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa"}]}"#,
        r#"{"intensity":0,"blend":[{"id":"anger","w":0.5},{"id":"sadness","w":0.5}]}"#,
        "[scales]\nf1_hz = 10\nrate = 0.2\n",
    );
    assert_eq!(plan.ticks[0].feeling, "mixture");
    assert_eq!(plan.ticks[0].f1.direction, "cancelled");
    close(plan.ticks[0].f1.amount.unwrap(), 0.0);
    assert_eq!(plan.ticks[0].rate.direction, "cancelled");
    close(plan.ticks[0].rate.amount.unwrap(), 1.0);
}

#[test]
fn phones_keep_their_visemes_and_posture() {
    let plan = one(
        r#"{"phones":[
            {"symbol":"p"},{"symbol":"iy1"},{"symbol":"aa"},
            {"symbol":"uw"},{"symbol":"f"},{"symbol":"l"},{"symbol":"sil"},
            {"symbol":"zz"}
        ]}"#,
        r#"{"emotion":"neutral","intensity":1}"#,
        "",
    );
    let shapes: Vec<_> = plan.avatar.ticks.iter().map(String::as_str).collect();
    assert_eq!(shapes, vec!["A", "B", "D", "F", "G", "H", "X", "X"]);
    assert_eq!(plan.ticks[1].phone, "iy");
    assert!(plan.ticks[1].phone_known);
    assert_eq!(plan.ticks[1].lips, "spread");
    assert_eq!(plan.ticks[1].tongue_front, "front");
    assert_eq!(plan.ticks[1].tongue_height, "high");
    assert_eq!(plan.ticks[3].lips, "round");
    assert_eq!(plan.ticks[3].tongue_front, "back");
    assert_eq!(plan.ticks[5].tongue_tip, "up");
    assert!(!plan.ticks[7].phone_known);
    assert!(plan.ticks.iter().all(|t| t.jaw_blended.is_none()));
    assert_eq!(plan.avatar.cues.len(), 8);
    assert!(plan.avatar.cues.iter().all(|c| c.end_ms - c.start_ms == 80));
}

#[test]
fn coarticulation_pulls_across_a_consonant_only_when_configured() {
    let phones = r#"{"phones":[{"symbol":"iy"},{"symbol":"p"},{"symbol":"aa"}]}"#;
    let bare = one(phones, r#"{"emotion":"neutral","intensity":1}"#, "");
    assert!(bare.ticks.iter().all(|t| t.jaw_blended.is_none()));
    assert_eq!(bare.ticks[1].viseme, "A");
    assert_eq!(bare.ticks[0].jaw, "close");
    assert_eq!(bare.ticks[2].jaw, "open");

    let mixed = one(
        phones,
        r#"{"emotion":"neutral","intensity":1}"#,
        "[scales]\ncoarticulation = 0.5\n",
    );
    close(mixed.ticks[0].jaw_blended.unwrap(), 0.0);
    close(mixed.ticks[1].jaw_blended.unwrap(), 0.0);
    close(mixed.ticks[2].jaw_blended.unwrap(), 0.0);
    close(mixed.ticks[0].tongue_front_blended.unwrap(), 0.0);
    close(mixed.ticks[0].tongue_height_blended.unwrap(), 0.0);
    // iy lips +1 pulled halfway toward aa lips 0. The consonant takes half of that average.
    close(mixed.ticks[0].lips_blended.unwrap(), 0.5);
    close(mixed.ticks[1].lips_blended.unwrap(), 0.25);
    close(mixed.ticks[2].lips_blended.unwrap(), 0.5);
    assert_eq!(mixed.ticks[0].lips, "spread");
    assert_eq!(mixed.ticks[1].viseme, "A");
    assert_eq!(mixed.ticks[2].lips, "neutral");
    assert!(!mixed.unknown.iter().any(|k| k == "coarticulation"));
}

#[test]
fn precision_scales_the_weight_only_when_its_size_is_set() {
    let phones =
        r#"{"phones":[{"symbol":"iy"},{"symbol":"p"},{"symbol":"aa"}],"precision":"clear"}"#;
    let hyper = one(
        phones,
        r#"{"emotion":"neutral","intensity":1}"#,
        "[scales]\ncoarticulation = 1\nprecision = 1\n",
    );
    close(hyper.ticks[0].jaw_blended.unwrap(), -1.0);
    assert!(hyper.ticks[1].jaw_blended.is_none());
    assert_eq!(hyper.ticks[0].precision, "hyper");
    close(hyper.ticks[0].precision_amount.unwrap(), 1.0);
    assert_eq!(hyper.ticks[0].viseme, "B");

    let slur = one(
        r#"{"phones":[{"symbol":"iy"},{"symbol":"p"},{"symbol":"aa"}],"precision":"slur"}"#,
        r#"{"emotion":"neutral","intensity":1}"#,
        "[scales]\ncoarticulation = 0\nprecision = 1\n",
    );
    close(slur.ticks[0].jaw_blended.unwrap(), 1.0);
    close(slur.ticks[0].tongue_front_blended.unwrap(), -1.0);
    close(slur.ticks[0].tongue_height_blended.unwrap(), -1.0);
    close(slur.ticks[1].jaw_blended.unwrap(), 0.0);
    close(slur.ticks[2].jaw_blended.unwrap(), -1.0);
    close(slur.ticks[2].tongue_front_blended.unwrap(), 1.0);
    assert_eq!(slur.ticks[1].viseme, "A");

    let size_unknown = one(
        phones,
        r#"{"emotion":"neutral","intensity":1}"#,
        "[scales]\ncoarticulation = 0.5\n",
    );
    assert_eq!(size_unknown.ticks[0].precision, "hyper");
    assert!(size_unknown.ticks[0].precision_amount.is_none());
    close(size_unknown.ticks[0].jaw_blended.unwrap(), 0.0);
    assert!(size_unknown.unknown.iter().any(|k| k == "precision"));
}

#[test]
fn faster_rate_shortens_cues_and_does_not_change_coarticulation() {
    let fast = one(
        r#"{"phones":[{"symbol":"iy","duration_ms":160},{"symbol":"aa","duration_ms":160}]}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\nrate = 1\ncoarticulation = 0.5\n",
    );
    assert_eq!(
        fast.avatar.cues[0].end_ms - fast.avatar.cues[0].start_ms,
        80
    );
    assert_eq!(
        fast.avatar.cues[1].end_ms - fast.avatar.cues[1].start_ms,
        80
    );
    close(fast.clone_mouth.speaking_rate.amount.unwrap(), 2.0);
    close(fast.clone_mouth.coarticulation_weight.unwrap(), 0.5);
    close(fast.ticks[0].coarticulation_weight.unwrap(), 0.5);

    let unscaled = one(
        r#"{"phones":[{"symbol":"iy","duration_ms":160},{"symbol":"aa","duration_ms":160}]}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\ncoarticulation = 0.5\n",
    );
    assert_eq!(unscaled.avatar.cues[0].end_ms, 160);
    assert_eq!(unscaled.ticks[0].rate.direction, "faster");
    assert!(unscaled.ticks[0].rate.amount.is_none());
    assert_eq!(unscaled.n_ticks, 4);
}

#[test]
fn a_later_feeling_does_not_rebuild_the_clock() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa","duration_ms":160},{"symbol":"iy","duration_ms":160}]}"#,
        r#"[{"emotion":"anger","intensity":1,"t_ms":0},{"emotion":"sadness","intensity":1,"t_ms":80}]"#,
        "[scales]\nrate = 0.25\nf1_hz = 8\n",
    );
    close(plan.clone_mouth.speaking_rate.amount.unwrap(), 1.25);
    assert_eq!(plan.avatar.cues[0].end_ms, 128);
    assert_eq!(plan.avatar.cues[1].end_ms, 256);
    assert_eq!(plan.ticks[0].f1.direction, "higher");
    close(plan.ticks[0].rate.amount.unwrap(), 1.25);
    // Tick centre 120 ms is already on sadness, still inside the first phone.
    assert_eq!(plan.ticks[1].feeling, "sadness");
    assert_eq!(plan.ticks[1].f1.direction, "lower");
    close(plan.ticks[1].rate.amount.unwrap(), 0.75);
    assert_eq!(plan.ticks[1].phone, "aa");
    assert_eq!(plan.clone_mouth.f1.direction, "varies");
}

#[test]
fn tail_tick_is_rest_and_still_carries_the_feeling() {
    let plan = one(
        r#"{"phones":[{"symbol":"aa","duration_ms":100}]}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\nf1_hz = 10\n",
    );
    assert_eq!(plan.n_ticks, 2);
    assert_eq!(plan.ticks[0].viseme, "D");
    close(plan.ticks[0].f1.amount.unwrap(), 10.0);
    assert_eq!(plan.ticks[1].viseme, "X");
    assert_eq!(plan.ticks[1].phone, "sil");
    close(plan.ticks[1].f1.amount.unwrap(), 10.0);
}

#[test]
fn empty_phones_are_one_rest_tick_and_plans_match() {
    let a = one(r#"{"text":"hello"}"#, "[]", "");
    let b = one(r#"{"text":"hello"}"#, "[]", "");
    assert_eq!(a.n_ticks, 1);
    assert_eq!(a.avatar.ticks, vec!["X".to_string()]);
    assert_eq!(a.intent.phone_count, 0);
    assert_eq!(a.intent.text, "hello");
    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb);
}

#[test]
fn non_positive_rate_is_rejected() {
    let err = plan(
        &intent(r#"{"phones":[{"symbol":"aa"}]}"#),
        &feelings(r#"{"emotion":"sadness","intensity":1}"#),
        &with_scales("[scales]\nrate = 1\n"),
    );
    assert!(matches!(err, Err(MouthError::Rate(_))));
    let bad = parse_scales("[timing]\nphone_ms = 80\n[scales]\ncoarticulation = 1.5\n");
    assert!(bad.is_err());
    let axis =
        parse_feelings(&serde_json::json!({"emotion":"anger","intensity":1.0,"arousal":2.0}));
    assert!(matches!(axis, Err(MouthError::BadNumber("arousal"))));
}

#[test]
fn read_card_and_worker_bundle_and_listen() {
    let card = r#"{
      "articulation": {
        "schema": 1, "tick_ms": 80, "n_ticks": 2, "duration_s": 0.16,
        "tracks": {
          "f1_hz": {"section":"mouth","unit":"Hz","source":"praatfan formant burg","ticks":[400.0, null]},
          "ema_ttx": {"section":"mouth","unit":"SPARC template units","source":"SPARC ema","ticks":[0.2, -0.4]},
          "phon_labial": {"section":"mouth","unit":"posterior","source":"Phonet","ticks":[0.8, 0.1]},
          "phon_nasal": {"section":"nose","unit":"posterior","source":"Phonet","ticks":[0.05, 0.9]}
        },
        "visemes": ["A", "D"],
        "bricks": [{"name":"praatfan_rust"},{"name":"SPARC"},{"name":"Phonet"},{"name":"Rhubarb Lip Sync"}]
      }
    }"#;
    let read = read_json(card).unwrap();
    assert_eq!(read.f1_hz, vec![Some(400.0), None]);
    assert_eq!(read.ema.get("ttx").unwrap(), &vec![Some(0.2), Some(-0.4)]);
    assert_eq!(read.phonet["nasal"].section, "nose");
    assert_eq!(read.phonet["labial"].section, "mouth");
    assert_eq!(read.visemes[0].as_deref(), Some("A"));
    assert!(read.note.contains("not nasalance"));

    let bundle = r#"{
      "duration_s": 0.16,
      "sparc": {
        "rate_hz": 50,
        "ema_names": ["ULY", "LLY"],
        "ema": [[1,0],[1,0],[1,0],[1,0],[3,1],[3,1],[3,1],[3,1]]
      },
      "disvoice": {"phonet": {"rate_hz": 100, "classes": {"vocalic": [0.2, 0.4]}}},
      "rhubarb": {"mouthCues": [
        {"start": 0.0, "end": 0.08, "value": "A"},
        {"start": 0.08, "end": 0.16, "value": "D"}
      ]}
    }"#;
    let raw = read_json(bundle).unwrap();
    assert_eq!(raw.n_ticks, 2);
    close(raw.lip_aperture[0].unwrap(), 1.0);
    close(raw.lip_aperture[1].unwrap(), 2.0);
    assert_eq!(raw.visemes[0].as_deref(), Some("A"));
    assert_eq!(raw.visemes[1].as_deref(), Some("D"));
    assert_eq!(raw.phonet["vocalic"].section, "mouth");
    assert!(raw.phonet["vocalic"].ticks.iter().any(|v| v.is_some()));

    let planned = one(
        r#"{"phones":[{"symbol":"p"},{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "[scales]\nf1_hz = 10\n",
    );
    let base = read_json(
        r#"{"articulation":{"tick_ms":80,"n_ticks":2,"tracks":{"f1_hz":{"ticks":[400.0, 500.0]}},"visemes":["A","D"]}}"#,
    )
    .unwrap();
    let heard = read_json(
        r#"{"articulation":{"tick_ms":80,"n_ticks":2,"tracks":{"f1_hz":{"ticks":[410.0, 520.0]}},"visemes":["A","C"]}}"#,
    )
    .unwrap();
    let report = listen(&planned, &heard, Some(&base));
    assert!(report.note.contains("Not a gate"));
    close(report.ticks[0].f1_error_hz.unwrap(), 0.0);
    close(report.ticks[1].f1_error_hz.unwrap(), 10.0);
    assert_eq!(report.ticks[0].viseme_match, Some(true));
    assert_eq!(report.ticks[1].viseme_match, Some(false));

    let no_base = listen(&planned, &heard, None);
    assert!(no_base.ticks[0].f1_error_hz.is_none());
    assert_eq!(no_base.ticks[0].measured_f1_hz, Some(410.0));

    let unknown = one(
        r#"{"phones":[{"symbol":"p"},{"symbol":"aa"}]}"#,
        r#"{"emotion":"anger","intensity":1}"#,
        "",
    );
    let unknown_listen = listen(&unknown, &heard, Some(&base));
    assert!(unknown_listen.ticks[0].f1_error_hz.is_none());
    assert!(unknown_listen.ticks[0].target_f1_hz.is_none());
}

#[test]
fn read_via_binary_parses_a_local_stub() {
    let dir = std::env::temp_dir().join(format!("galion-mouth-stub-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let card = dir.join("card.json");
    fs::write(
        &card,
        r#"{"articulation":{"schema":1,"tick_ms":80,"n_ticks":1,"duration_s":0.08,"tracks":{"f1_hz":{"section":"mouth","unit":"Hz","source":"praatfan formant burg","ticks":[440.0]}},"visemes":["B"],"bricks":[{"name":"praatfan_rust"}]}}"#,
    )
    .unwrap();
    let bin = dir.join("fake-articulation");
    fs::write(
        &bin,
        format!(
            "#!/bin/sh\nout=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--out\" ]; then out=\"$2\"; shift 2; else shift; fi\ndone\ncat '{}' > \"$out\"\n",
            card.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let read = read_via_binary(&bin, &dir.join("not-a-real-recording.wav")).unwrap();
    assert_eq!(read.f1_hz, vec![Some(440.0)]);
    assert_eq!(read.visemes[0].as_deref(), Some("B"));
    assert!(read.bricks.iter().any(|b| b == "praatfan_rust"));
    let _ = fs::remove_dir_all(&dir);
}
