// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic checks only. No audio, no dataset, no measuring run.

use std::fs;
use std::path::PathBuf;

use galion_emotion_render::{
    check_inventory_file, clone_request_patch, load_scales, render_tick, render_utterance,
    render_word, resolve_name, speech_directions, Category, FeatureScales, RenderError, Resolved,
    Scales, Sign, TargetTick, Track, CATEGORIES, TICK_MS,
};
use serde_json::json;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn engine() -> Scales {
    load_scales(&repo().join("configs/symphony/emotion-render.toml")).unwrap()
}

fn synthetic_steps() -> Scales {
    let mut scales = engine();
    scales.shared = FeatureScales {
        pitch_st: Some(2.0),
        rate: Some(0.1),
        energy: Some(0.2),
        pause_ms: Some(100.0),
        pitch_range: Some(0.1),
        exaggeration: None,
        cfg_weight: None,
        temperature: None,
    };
    scales
}

fn named(cat: Category, track: Track, intensity: f64, dominance: f64) -> TargetTick {
    let mut tick = TargetTick::named(
        cat,
        track,
        intensity,
        galion_emotion_render::Avd {
            arousal: 0.0,
            valence: 0.0,
            dominance,
        },
    )
    .unwrap();
    tick.t_ms = TICK_MS;
    tick
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn inventory_file_matches_the_crate() {
    check_inventory_file(&repo().join("configs/emotions-v2.toml")).unwrap();
    assert_eq!(CATEGORIES.len(), 24);
    assert_eq!(TICK_MS, 80);
    let reliable = CATEGORIES
        .iter()
        .filter(|c| c.tier().as_str() == "cross-culturally reliable in prosody")
        .count();
    assert_eq!(reliable, 12);
}

#[test]
fn shipped_magnitudes_are_unknown_and_engine_literals_match_the_worker() {
    let scales = engine();
    assert!(scales.shared.pitch_st.is_none());
    assert!(scales.shared.rate.is_none());
    assert!(scales.shared.energy.is_none());
    assert!(scales.shared.pause_ms.is_none());
    assert!(scales.shared.pitch_range.is_none());
    assert!(scales.shared.exaggeration.is_none());
    assert!(scales.shared.cfg_weight.is_none());
    assert!(scales.shared.temperature.is_none());
    for row in scales.poles.values() {
        assert!(row.pitch_st.is_none());
        assert!(row.rate.is_none());
        assert!(row.energy.is_none());
    }
    let worker = fs::read_to_string(repo().join("scripts/clone/chatterbox_worker.py")).unwrap();
    let nano = fs::read_to_string(repo().join("scripts/clone/nano_stream.py")).unwrap();
    assert!(worker.contains("exaggeration=0.0"));
    assert!(nano.contains("temperature = 0.8"));
    assert!(nano.contains("cfg_weight=0.0"));
    approx(scales.engine.exaggeration, 0.0);
    approx(scales.engine.cfg_weight, 0.0);
    approx(scales.engine.temperature, 0.8);
}

#[test]
fn every_category_is_a_legal_prosody_and_a_legal_burst() {
    let scales = engine();
    for cat in CATEGORIES {
        for track in [Track::Prosody, Track::Burst] {
            let tick = named(cat, track, 1.0, 0.0);
            let out = render_tick(&tick, &scales).unwrap();
            assert_eq!(out.emotion, cat.as_str());
            assert_eq!(out.track, track.as_str());
            assert_eq!(out.tick_ms, 80);
            assert_eq!(out.relative_to, "speaker_own_neutral");
            assert!(!out.applied.pitch_st);
            assert!(!out.applied.exaggeration);
            approx(out.prosody.pitch_st, 0.0);
            approx(out.prosody.rate, 1.0);
            approx(out.prosody.energy, 1.0);
            assert_eq!(out.prosody.pre_pause_ms, 0);
            approx(out.chatterbox.exaggeration, 0.0);
            approx(out.chatterbox.cfg_weight, 0.0);
            approx(out.chatterbox.temperature, 0.8);
            assert!(out.open.iter().any(|s| s == "valence_not_a_feature"));
            assert!(out.open.iter().any(|s| s == "arousal_no_per_unit"));
            assert!(out.citations.iter().all(|c| !c.contains("1986")));
        }
    }
}

#[test]
fn a_word_can_be_said_with_embarrassment() {
    let scales = engine();
    let tick = named(Category::Embarrassment, Track::Prosody, 1.0, 0.0);
    let word = render_word("hello", &tick, &scales).unwrap();
    assert_eq!(word.text, "hello");
    assert_eq!(word.controls.emotion, "embarrassment");
    assert_eq!(
        word.controls.prosody_tier.as_deref(),
        Some("valid in prosody, lower recognition evidence")
    );
    assert_eq!(word.controls.directions.pitch_st, "unknown");
    assert_eq!(word.controls.directions.rate, "unknown");
    assert_eq!(word.controls.directions.energy, "unknown");
    assert_eq!(word.controls.directions.pause, "unknown");
    let patch = clone_request_patch(&word.controls);
    assert_eq!(patch["emotion"], "embarrassment");
    assert_eq!(patch["prosody"]["rate"], 1.0);
    assert_eq!(patch["exaggeration"], 0.0);
}

#[test]
fn anger_and_sadness_signs_follow_juslin_and_banse() {
    let hot = speech_directions(Category::Anger, 1.0);
    let cold = speech_directions(Category::Anger, -1.0);
    let modal = speech_directions(Category::Anger, 0.0);
    assert_eq!(hot.pole, Some("hot_anger"));
    assert_eq!(cold.pole, Some("cold_anger"));
    assert_eq!(modal.pole, Some("anger_modal"));
    for row in [hot, cold, modal] {
        assert_eq!(row.pitch.sign, Sign::Higher);
        assert_eq!(row.pitch_range.sign, Sign::Higher);
        assert_eq!(row.rate.sign, Sign::Higher);
        assert_eq!(row.energy.sign, Sign::Higher);
        assert_eq!(row.pause.sign, Sign::Lower);
    }
    let sad = speech_directions(Category::Sadness, 0.0);
    assert_eq!(sad.pitch.sign, Sign::Lower);
    assert_eq!(sad.rate.sign, Sign::Lower);
    assert_eq!(sad.energy.sign, Sign::Lower);
    assert_eq!(sad.pause.sign, Sign::Higher);
    let labels: toml::Value = toml::from_str(
        &fs::read_to_string(repo().join("configs/symphony/emotion-labels-v2.toml")).unwrap(),
    )
    .unwrap();
    let anger = &labels["regions"]["anger"];
    assert_eq!(anger["juslin_modal"]["f0_mean"].as_str(), Some("higher"));
    assert_eq!(anger["juslin_modal"]["pause"].as_str(), Some("fewer"));
    assert!(anger["banse_high_pole"]["f0_mean_z"].as_float().unwrap() > 0.0);
    assert!(anger["banse_low_pole"]["f0_mean_z"].as_float().unwrap() > 0.0);
    assert!(
        anger["banse_high_pole"]["articulation_duration_z"]
            .as_float()
            .unwrap()
            < 0.0
    );
    let sadness = &labels["regions"]["sadness"];
    assert_eq!(sadness["juslin_modal"]["rate"].as_str(), Some("slower"));
    assert_eq!(sadness["juslin_modal"]["pause"].as_str(), Some("more"));
    assert!(sadness["banse_low_pole"]["loudness_z"].as_float().unwrap() < 0.0);
    assert!(sadness["banse_high_pole"]["f0_mean_z"].as_float().unwrap() > 0.0);
}

#[test]
fn fear_poles_do_not_collapse() {
    let panic = speech_directions(Category::Fear, 0.4);
    let anxiety = speech_directions(Category::Fear, -0.2);
    let modal = speech_directions(Category::Fear, 0.0);
    assert_eq!(panic.pole, Some("panic"));
    assert_eq!(panic.pitch.sign, Sign::Higher);
    assert_eq!(panic.energy.sign, Sign::Higher);
    assert_eq!(panic.rate.sign, Sign::Higher);
    assert_eq!(anxiety.pole, Some("anxiety"));
    assert_eq!(anxiety.pitch.sign, Sign::Lower);
    assert_eq!(anxiety.energy.sign, Sign::Lower);
    assert_eq!(anxiety.rate.sign, Sign::Higher);
    assert_eq!(modal.pole, Some("fear_modal"));
    assert_eq!(modal.pitch.sign, Sign::Higher);
    assert_eq!(modal.energy.sign, Sign::Unresolved);
    assert_eq!(modal.pitch_range.sign, Sign::Unresolved);
    assert_eq!(modal.pause.sign, Sign::Unresolved);
    let labels: toml::Value = toml::from_str(
        &fs::read_to_string(repo().join("configs/symphony/emotion-labels-v2.toml")).unwrap(),
    )
    .unwrap();
    let fear = &labels["regions"]["fear"];
    assert_eq!(fear["juslin_modal"]["loudness"].as_str(), Some("bimodal"));
    assert_eq!(
        fear["juslin_modal"]["f0_range"].as_str(),
        Some("inconsistent")
    );
    assert!(fear["banse_high_pole"]["f0_mean_z"].as_float().unwrap() > 0.0);
    assert!(fear["banse_low_pole"]["f0_mean_z"].as_float().unwrap() < 0.0);
    assert!(
        fear["banse_low_pole"]["articulation_duration_z"]
            .as_float()
            .unwrap()
            < 0.0
    );
}

#[test]
fn configured_step_uses_the_sign_and_the_weight() {
    let scales = synthetic_steps();
    let anger = render_tick(&named(Category::Anger, Track::Prosody, 1.0, 1.0), &scales).unwrap();
    assert_eq!(anger.directions.pitch_st, "higher");
    assert_eq!(anger.directions.rate, "faster");
    assert_eq!(anger.directions.energy, "higher");
    assert_eq!(anger.directions.pause, "fewer");
    assert!(anger.applied.pitch_st);
    assert!(anger.applied.rate);
    assert!(anger.applied.energy);
    assert!(!anger.applied.pause);
    approx(anger.prosody.pitch_st, 2.0);
    approx(anger.prosody.rate, 1.1);
    approx(anger.prosody.energy, 1.2);
    assert_eq!(anger.prosody.pre_pause_ms, 0);
    assert!(anger
        .open
        .iter()
        .any(|s| s == "pause_fewer_not_pre_pause_ms"));
    assert!(!anger.applied.exaggeration);
    approx(anger.chatterbox.temperature, 0.8);

    let sad = render_tick(&named(Category::Sadness, Track::Prosody, 0.5, 0.0), &scales).unwrap();
    assert_eq!(sad.directions.rate, "slower");
    assert_eq!(sad.directions.pause, "more");
    approx(sad.prosody.pitch_st, -1.0);
    approx(sad.prosody.rate, 0.95);
    approx(sad.prosody.energy, 0.9);
    assert_eq!(sad.prosody.pre_pause_ms, 50);
    assert!(sad.applied.pause);
}

#[test]
fn unknown_sign_does_not_move_when_a_step_exists() {
    let scales = synthetic_steps();
    let out = render_tick(
        &named(Category::Embarrassment, Track::Prosody, 1.0, 0.0),
        &scales,
    )
    .unwrap();
    assert!(!out.applied.pitch_st);
    approx(out.prosody.pitch_st, 0.0);
    approx(out.prosody.rate, 1.0);
    assert!(out.open.iter().any(|s| s == "sign_unknown:pitch_st"));
    let zero = render_tick(&named(Category::Anger, Track::Prosody, 0.0, 1.0), &scales).unwrap();
    assert_eq!(zero.emotion, "neutral");
    approx(zero.prosody.pitch_st, 0.0);
}

#[test]
fn valence_does_not_move_a_knob_and_arousal_is_not_a_gain() {
    let scales = synthetic_steps();
    let mut up = named(Category::Sadness, Track::Prosody, 1.0, 0.0);
    up.avd.valence = 1.0;
    up.avd.arousal = 1.0;
    let mut down = named(Category::Sadness, Track::Prosody, 1.0, 0.0);
    down.avd.valence = -1.0;
    down.avd.arousal = -1.0;
    let a = render_tick(&up, &scales).unwrap();
    let b = render_tick(&down, &scales).unwrap();
    assert_eq!(a.prosody, b.prosody);
    assert_eq!(a.chatterbox, b.chatterbox);
    assert_eq!(a.directions, b.directions);
    assert_eq!(a.applied, b.applied);
}

#[test]
fn fear_modal_leaves_bimodal_loudness_alone() {
    let scales = synthetic_steps();
    let out = render_tick(&named(Category::Fear, Track::Prosody, 1.0, 0.0), &scales).unwrap();
    assert_eq!(out.directions.energy, "unresolved");
    assert!(!out.applied.energy);
    approx(out.prosody.energy, 1.0);
    assert!(out.applied.pitch_st);
    approx(out.prosody.pitch_st, 2.0);
    let anxious = render_tick(&named(Category::Fear, Track::Prosody, 1.0, -1.0), &scales).unwrap();
    assert_eq!(anxious.pole.as_deref(), Some("anxiety"));
    approx(anxious.prosody.pitch_st, -2.0);
    approx(anxious.prosody.energy, 0.8);
    approx(anxious.prosody.rate, 1.1);
}

#[test]
fn pole_step_replaces_the_shared_step() {
    let mut scales = synthetic_steps();
    scales.poles.insert(
        "panic".into(),
        FeatureScales {
            pitch_st: Some(3.0),
            rate: None,
            energy: None,
            pause_ms: None,
            pitch_range: None,
            exaggeration: None,
            cfg_weight: None,
            temperature: None,
        },
    );
    let out = render_tick(&named(Category::Fear, Track::Prosody, 1.0, 1.0), &scales).unwrap();
    assert_eq!(out.pole.as_deref(), Some("panic"));
    approx(out.prosody.pitch_st, 3.0);
    approx(out.prosody.rate, 1.1);
}

#[test]
fn decoder_json_weight_is_the_mixture() {
    let scales = synthetic_steps();
    let v = json!({
        "t_ms": 160,
        "arousal": 0.0,
        "valence": 0.0,
        "dominance": 0.0,
        "intensity": 1.0,
        "blend": [
            {"id": "sadness", "w": 0.5},
            {"id": "anger", "w": 0.5},
            {"id": "neutral", "w": 0.0}
        ]
    });
    let tick = TargetTick::from_decoder_value(&v).unwrap();
    let out = render_tick(&tick, &scales).unwrap();
    assert_eq!(out.t_ms, 160);
    assert_eq!(out.directions.pitch_st, "mixed");
    approx(out.prosody.pitch_st, 0.0);
    assert!(out.applied.pitch_st);
    let only = json!({
        "emotion": "sadness",
        "intensity": 0.5,
        "dominance": 0.0
    });
    let one = render_tick(&TargetTick::from_decoder_value(&only).unwrap(), &scales).unwrap();
    approx(one.prosody.pitch_st, -1.0);
}

#[test]
fn burst_keeps_the_category_and_does_not_apply_speech_signs() {
    let scales = synthetic_steps();
    let out = render_tick(&named(Category::Anger, Track::Burst, 1.0, 1.0), &scales).unwrap();
    assert_eq!(out.emotion, "anger");
    assert_eq!(out.track, "burst");
    assert_eq!(out.directions.pitch_st, "higher");
    assert!(!out.applied.pitch_st);
    approx(out.prosody.pitch_st, 0.0);
    assert!(out.open.iter().any(|s| s == "burst_withholds_speech"));
    let laugh =
        TargetTick::from_name("laughter", None, 1.0, galion_emotion_render::Avd::zero()).unwrap();
    let burst = render_tick(&laugh, &scales).unwrap();
    assert_eq!(burst.emotion, "amusement");
    assert_eq!(burst.track, "burst");
    assert_eq!(burst.alias_from.as_deref(), Some("laughter"));
}

#[test]
fn cited_rows_that_are_not_the_big_five() {
    let scales = synthetic_steps();
    let labels: toml::Value = toml::from_str(
        &fs::read_to_string(repo().join("configs/symphony/emotion-labels-v2.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        labels["regions"]["adoration"]["juslin_modal"]["f0_mean"].as_str(),
        Some("lower")
    );
    assert_eq!(
        labels["regions"]["adoration"]["juslin_modal"]["pause"].as_str(),
        Some("unknown, to be learned from data")
    );
    assert_eq!(
        labels["regions"]["desire"]["f0_mean"].as_str(),
        Some("lower")
    );
    assert!(
        labels["regions"]["disgust"]["f0_mean_z"]
            .as_float()
            .unwrap()
            < 0.0
    );
    assert!(
        labels["regions"]["disgust"]["loudness_z"]
            .as_float()
            .unwrap()
            < 0.0
    );
    let elation = &labels["legacy_evidence"]["happiness"]["banse_high_pole"];
    assert_eq!(elation["name"].as_str(), Some("elation"));
    assert!(elation["articulation_duration_z"].as_float().unwrap() > 0.0);
    assert_eq!(
        labels["regions"]["amusement"]["speech"].as_str(),
        Some("unknown, to be learned from data")
    );

    let tender = speech_directions(Category::Adoration, 0.0);
    assert_eq!(tender.pitch.sign, Sign::Lower);
    assert_eq!(tender.pause.sign, Sign::Unknown);
    let desire = render_tick(&named(Category::Desire, Track::Prosody, 1.0, 0.0), &scales).unwrap();
    assert_eq!(desire.directions.pitch_st, "lower");
    assert_eq!(desire.directions.rate, "unknown");
    approx(desire.prosody.pitch_st, -2.0);
    approx(desire.prosody.rate, 1.0);
    let disgust =
        render_tick(&named(Category::Disgust, Track::Prosody, 1.0, 0.0), &scales).unwrap();
    assert_eq!(disgust.directions.pitch_st, "lower");
    assert_eq!(disgust.directions.energy, "lower");
    assert_eq!(disgust.directions.pause, "more");
    assert_eq!(disgust.prosody.pre_pause_ms, 100);
    let el = render_tick(&named(Category::Elation, Track::Prosody, 1.0, 0.0), &scales).unwrap();
    assert_eq!(el.directions.pitch_st, "higher");
    assert_eq!(el.directions.energy, "higher");
    assert_eq!(el.directions.rate, "slower");
    approx(el.prosody.rate, 0.9);
    let amusement = render_tick(
        &named(Category::Amusement, Track::Prosody, 1.0, 0.0),
        &scales,
    )
    .unwrap();
    assert_eq!(amusement.directions.pitch_st, "unknown");
    assert!(!amusement.applied.pitch_st);
}

#[test]
fn old_names_follow_the_crosswalk_and_non_categories_fail() {
    match resolve_name("seductive").unwrap() {
        Resolved::One { category, .. } => assert_eq!(category, Category::Desire),
        other => panic!("{other:?}"),
    }
    match resolve_name("disgusted").unwrap() {
        Resolved::One { category, .. } => assert_eq!(category, Category::Contempt),
        other => panic!("{other:?}"),
    }
    match resolve_name("calm").unwrap() {
        Resolved::Baseline => {}
        other => panic!("{other:?}"),
    }
    match resolve_name("happy") {
        Err(RenderError::Split { options, .. }) => {
            assert_eq!(
                options,
                vec!["amusement".to_string(), "interest".to_string()]
            );
        }
        other => panic!("{other:?}"),
    }
    match resolve_name("sigh") {
        Err(RenderError::BurstDetector { may_be, .. }) => {
            assert_eq!(may_be.len(), 4);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        resolve_name("guilt"),
        Err(RenderError::NotACategory(_))
    ));
    assert!(matches!(
        resolve_name("serenity"),
        Err(RenderError::NotACategory(_))
    ));
    let reliable =
        render_tick(&named(Category::Anger, Track::Prosody, 1.0, 0.0), &engine()).unwrap();
    assert_eq!(
        reliable.prosody_tier.as_deref(),
        Some("cross-culturally reliable in prosody")
    );
}

#[test]
fn utterance_median_is_the_chatterbox_triple_plus_rate() {
    let scales = synthetic_steps();
    let ticks = [
        named(Category::Sadness, Track::Prosody, 1.0, 0.0),
        named(Category::Anger, Track::Prosody, 1.0, 1.0),
    ];
    let utt = render_utterance(&ticks, &scales).unwrap();
    assert_eq!(utt.ticks.len(), 2);
    assert_eq!(utt.tick_ms, 80);
    approx(utt.chatterbox.speed, 1.0);
    approx(utt.chatterbox.exaggeration, 0.0);
    approx(utt.chatterbox.cfg, utt.chatterbox.cfg_weight);
    assert!(render_utterance(&[], &scales).is_err());
}

#[test]
fn crate_tree_has_no_audio_files() {
    fn walk(dir: &std::path::Path) {
        for ent in fs::read_dir(dir).unwrap() {
            let ent = ent.unwrap();
            let path = ent.path();
            if path.file_name().unwrap() == "target" {
                continue;
            }
            if path.is_dir() {
                walk(&path);
            } else {
                let name = path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                for ext in [".wav", ".mp3", ".flac", ".ogg"] {
                    assert!(!name.ends_with(ext), "{}", path.display());
                }
            }
        }
    }
    walk(&PathBuf::from(env!("CARGO_MANIFEST_DIR")));
}
