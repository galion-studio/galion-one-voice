// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic signals only. No voice audio is read or written in the repository.

use galion_lungs::config::Config;
use galion_lungs::inventory::Inventory;
use galion_lungs::plan::{self, Moment, Request};
use galion_lungs::stage::{self, BreathCue};
use galion_lungs::synth::{self, Reference};
use galion_lungs::wav;
use galion_lungs::TICK_S;

fn cfg() -> Config {
    Config::bundled().expect("lungs config")
}

fn inventory() -> Inventory {
    Inventory::bundled().expect("emotion inventory")
}

fn planned(text: &str, category: &str, arousal: f64, energy: f64, fatigue: f64) -> plan::Plan {
    let req = Request::constant(text, category, arousal, energy, fatigue);
    plan::plan(&req, &cfg(), &inventory()).unwrap()
}

fn speech_ticks(p: &plan::Plan) -> Vec<&plan::Tick> {
    p.ticks.iter().filter(|t| t.segment == "speech").collect()
}

fn alphas(n: usize) -> String {
    let mut words = vec!["alpha"; n];
    if let Some(last) = words.last_mut() {
        *last = "alpha.";
    }
    words.join(" ")
}

#[test]
fn inventory_is_the_cowen_24_and_breath_is_not_one() {
    let inv = inventory();
    assert_eq!(inv.labels.len(), 24);
    assert!(inv.labels.iter().all(|s| s != "breath" && s != "neutral"));
    assert!(inv
        .check_state("breath")
        .unwrap_err()
        .contains("not an emotion"));
    assert!(inv.check_state("sigh").is_err());
    assert!(inv.check_state("laughter").is_err());
    assert!(inv.check_state("neutral").is_ok());
    for label in &inv.labels {
        inv.check_state(label).unwrap();
    }
}

#[test]
fn plan_is_physiology_on_the_80ms_grid() {
    let p = planned("Hello there.", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(p.schema, "galion-lungs/1");
    assert_eq!(p.kind, "physiology");
    assert!(p.not_emotion);
    assert_eq!(p.tick_ms, 80);
    assert_eq!(p.n_ticks, p.ticks.len());
    assert!((p.duration_s - p.n_ticks as f64 * TICK_S).abs() < 1e-9);
    for (i, t) in p.ticks.iter().enumerate() {
        assert_eq!(t.t_ms, i as u32 * 80);
        assert_ne!(t.category, "breath");
        assert_ne!(t.segment, "breath");
    }
    for s in &p.segments {
        let d = s.end_s - s.start_s;
        assert!((d / TICK_S).fract() < 1e-9 || (d / TICK_S).fract() > 1.0 - 1e-9);
    }
    let joined = p.directions.join(" ");
    for needle in [
        "Hixon",
        "Winkworth",
        "Whalen",
        "Ladefoged",
        "Titze",
        "Boiten",
        "Homma",
        "Cowen",
    ] {
        assert!(joined.contains(needle), "missing {needle}");
    }
    assert!(p.reuses.iter().any(|s| s.contains("bc-56b9f22c")));
}

#[test]
fn a_sentence_boundary_takes_an_inhalation_and_a_short_clause_does_not() {
    let two = planned("Hi. Hi.", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(
        two.phrases.len(),
        2,
        "each sentence is its own breath group"
    );
    assert_eq!(
        two.segments.iter().filter(|s| s.kind == "inhale").count(),
        2
    );

    let bare = planned("Hi Hi", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(bare.phrases.len(), 1);
    let lines = planned("Hi\nHi", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(
        lines.phrases.len(),
        2,
        "a line break is a breath-group boundary"
    );
    let empty = planned("   ", "neutral", 0.0, 0.5, 0.0);
    assert!(empty.phrases.is_empty());
    assert_eq!(empty.n_ticks, 0);
    assert!(empty.not_emotion);

    let short_clause = planned("one, two", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(
        short_clause.phrases.len(),
        1,
        "a short comma does not force a breath"
    );

    let long_clause = planned("one, two, three, four, five, six", "sadness", 0.0, 0.5, 0.0);
    assert!(
        long_clause.phrases.len() >= 2,
        "a long clause chain takes another inhalation"
    );
}

#[test]
fn a_longer_phrase_gets_a_longer_deeper_inspiration() {
    let short = planned("Hi.", "sadness", 0.0, 0.5, 0.0);
    let long = planned(&alphas(10), "sadness", 0.0, 0.5, 0.0);
    assert!(long.phrases[0].speech_s > short.phrases[0].speech_s);
    assert!(long.phrases[0].insp_s > short.phrases[0].insp_s);
    assert!(long.phrases[0].depth > short.phrases[0].depth);
    assert_eq!(long.phrases[0].form, "deep");
}

#[test]
fn volume_and_pressure_fall_across_the_phrase_and_lift_loudness_and_f0() {
    let p = planned(
        "Hello there friend from the hill.",
        "sadness",
        0.0,
        0.5,
        0.0,
    );
    let inhale: Vec<_> = p.ticks.iter().filter(|t| t.segment == "inhale").collect();
    assert!(inhale.len() >= 2);
    assert!(inhale.last().unwrap().lung_volume_rel > inhale.first().unwrap().lung_volume_rel);
    assert!(inhale.iter().all(|t| t.subglottal_pressure_rel.is_none()));
    assert!(inhale
        .iter()
        .all(|t| t.loudness_db.is_none() && t.f0_st.is_none()));

    let speech = speech_ticks(&p);
    assert!(speech.len() >= 4);
    let first = speech.first().unwrap();
    let last = speech.last().unwrap();
    assert!(first.lung_volume_rel > last.lung_volume_rel);
    let ps0 = first.subglottal_pressure_rel.unwrap();
    let ps1 = last.subglottal_pressure_rel.unwrap();
    assert!(ps0 > ps1);
    assert!(first.loudness_db.unwrap() > last.loudness_db.unwrap());
    assert!(first.f0_st.unwrap() > last.f0_st.unwrap());
    for w in speech.windows(2) {
        assert!(w[0].lung_volume_rel + 1e-12 >= w[1].lung_volume_rel);
        assert!(
            w[0].subglottal_pressure_rel.unwrap() + 1e-12 >= w[1].subglottal_pressure_rel.unwrap()
        );
    }

    let c = cfg();
    for t in &p.ticks {
        assert!(t.lung_volume_rel + 1e-9 >= c.volume.floor);
        assert!(t.lung_volume_rel <= c.volume.rest + c.volume.span + 1e-9);
    }
}

#[test]
fn energy_and_arousal_raise_pressure_and_breath_level() {
    let quiet = planned("Hello there.", "sadness", 0.0, 0.2, 0.0);
    let driven = planned("Hello there.", "sadness", 0.0, 0.9, 0.0);
    let mean = |p: &plan::Plan| {
        let s = speech_ticks(p);
        s.iter().map(|t| t.loudness_db.unwrap()).sum::<f64>() / s.len() as f64
    };
    assert!(mean(&driven) > mean(&quiet));
    assert!(
        speech_ticks(&driven)[0].subglottal_pressure_rel.unwrap()
            > speech_ticks(&quiet)[0].subglottal_pressure_rel.unwrap()
    );

    let low = planned("Hello there.", "interest", 0.0, 0.5, 0.0);
    let high = planned("Hello there.", "interest", 0.8, 0.5, 0.0);
    assert!(
        speech_ticks(&high)[0].loudness_db.unwrap() > speech_ticks(&low)[0].loudness_db.unwrap()
    );
    let inhale_db = |p: &plan::Plan| {
        p.segments
            .iter()
            .find(|s| s.kind == "inhale")
            .unwrap()
            .loudness_dbfs
            .unwrap()
    };
    assert!(inhale_db(&high) > inhale_db(&low));
    assert_eq!(high.phrases[0].cycle, "faster");
    assert_eq!(low.phrases[0].cycle, "structural");
}

#[test]
fn amusement_disgust_and_fear_follow_the_cited_directions() {
    let neutral = planned("Hello there.", "sadness", 0.0, 0.5, 0.0);
    let amused = planned("Hello there.", "amusement", 0.0, 0.5, 0.0);
    assert!(amused.phrases[0].insp_s < neutral.phrases[0].insp_s);
    assert!(amused.phrases[0].depth < neutral.phrases[0].depth);
    assert_eq!(amused.phrases[0].form, "short");
    assert_eq!(amused.phrases[0].hold_s, 0.0);
    assert!(amused.segments.iter().all(|s| s.kind != "hold"));

    let disgust = planned("Hello there.", "disgust", 0.0, 0.5, 0.0);
    assert!(disgust.phrases[0].hold_s > 0.0);
    assert!(disgust
        .segments
        .iter()
        .any(|s| s.kind == "hold" && s.form == "pause"));

    let fear = planned(&alphas(20), "fear", 0.0, 0.5, 0.0);
    let calm = planned(&alphas(20), "sadness", 0.0, 0.5, 0.0);
    assert!(fear.phrases.len() > calm.phrases.len());
    assert!(fear
        .phrases
        .iter()
        .all(|p| p.cycle == "faster" && p.form == "quick"));

    let aroused = planned(&alphas(20), "sadness", 1.0, 0.5, 0.0);
    assert!(aroused.phrases.len() > calm.phrases.len());
}

#[test]
fn categories_without_a_breath_row_share_the_structural_plan() {
    let a = planned("Hello there.", "awe", 0.0, 0.5, 0.0);
    let b = planned("Hello there.", "sadness", 0.0, 0.5, 0.0);
    assert_eq!(a.phrases[0].insp_s, b.phrases[0].insp_s);
    assert_eq!(a.phrases[0].depth, b.phrases[0].depth);
    assert_eq!(a.phrases[0].hold_s, b.phrases[0].hold_s);
    assert_eq!(a.phrases.len(), b.phrases.len());
    let desire = planned("Hello there.", "desire", 0.0, 0.5, 0.0);
    let exhale = |p: &plan::Plan| {
        p.segments
            .iter()
            .find(|s| s.kind == "exhale")
            .unwrap()
            .loudness_dbfs
    };
    assert_eq!(
        exhale(&desire),
        exhale(&b),
        "desire has no published exhale level"
    );

    let inv = inventory();
    for label in inv.labels {
        if label == "amusement" || label == "disgust" || label == "fear" {
            continue;
        }
        let p = planned("Hello there.", &label, 0.0, 0.5, 0.0);
        assert_eq!(p.phrases[0].insp_s, b.phrases[0].insp_s, "{label}");
        assert_eq!(p.phrases[0].depth, b.phrases[0].depth, "{label}");
        assert_eq!(p.phrases[0].hold_s, 0.0, "{label}");
        assert!(p.ticks.iter().all(|t| t.category == label));
    }
}

#[test]
fn fatigue_moves_with_its_config_coefficient_and_is_not_a_citation() {
    let c = cfg();
    assert_eq!(c.fatigue.literature, "none");
    let low = planned(&alphas(20), "contentment", 0.0, 0.5, 0.0);
    let high = planned(&alphas(20), "contentment", 0.0, 0.5, 1.0);
    let k = c.fatigue.group_per_unit;
    if k < 0.0 {
        assert!(high.phrases.len() > low.phrases.len());
    } else if k > 0.0 {
        assert!(high.phrases.len() < low.phrases.len());
    } else {
        assert_eq!(high.phrases.len(), low.phrases.len());
    }
    let d0 = planned("Hello there.", "contentment", 0.0, 0.5, 0.0).phrases[0].depth;
    let d1 = planned("Hello there.", "contentment", 0.0, 0.5, 1.0).phrases[0].depth;
    let kd = c.fatigue.depth_per_unit;
    if kd < 0.0 {
        assert!(d1 < d0);
    } else if kd > 0.0 {
        assert!(d1 > d0);
    } else {
        assert!((d1 - d0).abs() < 1e-12);
    }
}

#[test]
fn arousal_can_change_inside_the_utterance() {
    let mut req = Request::constant(
        "Hello there friend from the hill.",
        "sadness",
        0.0,
        0.5,
        0.0,
    );
    req.trajectory = vec![
        Moment {
            at_s: 0.0,
            category: None,
            arousal: Some(0.0),
            energy: None,
            fatigue: None,
        },
        Moment {
            at_s: 1.0,
            category: Some("fear".into()),
            arousal: Some(0.8),
            energy: None,
            fatigue: None,
        },
    ];
    let p = plan::plan(&req, &cfg(), &inventory()).unwrap();
    assert_eq!(p.phrases[0].category, "sadness");
    assert_ne!(p.phrases[0].form, "quick");
    let early = speech_ticks(&p)
        .into_iter()
        .find(|t| t.speech_t_s.unwrap() < 1.0)
        .unwrap();
    let late = speech_ticks(&p)
        .into_iter()
        .find(|t| t.speech_t_s.unwrap() >= 1.0)
        .unwrap();
    assert!((early.arousal - 0.0).abs() < 1e-12);
    assert_eq!(early.category, "sadness");
    assert!((late.arousal - 0.8).abs() < 1e-12);
    assert_eq!(late.category, "fear");
}

#[test]
fn synthesized_breaths_hit_the_planned_level_and_a_hold_is_silent() {
    let c = cfg();
    assert!((c.audible.neutral_rms - 0.003).abs() < 1e-12);
    assert_eq!(c.audible.seed, 13);
    assert!((c.audible.silence_floor_dbfs - (-55.0)).abs() < 1e-12);

    let p = planned("Hello there.", "disgust", 0.2, 0.7, 0.3);
    let sr = 16_000.0;
    let a = synth::render_track(&p, sr, None, &c).unwrap();
    let b = synth::render_track(&p, sr, None, &c).unwrap();
    assert_eq!(a.samples, b.samples);

    for ev in &a.events {
        let i0 = (ev.start_s * sr).round() as usize;
        let i1 = (ev.end_s * sr).round() as usize;
        let block = &a.samples[i0..i1];
        match ev.kind.as_str() {
            "inhale" | "exhale" => {
                assert_eq!(ev.source, "synthesized");
                let target = 10f64.powf(ev.loudness_dbfs.unwrap() / 20.0);
                let got = synth::rms(block);
                assert!(
                    (got - target).abs() / target < 1e-6,
                    "{} {got} vs {target}",
                    ev.kind
                );
                assert!(
                    lag1(block).abs() < 0.5,
                    "synthesized breath should not be a tone"
                );
            }
            "hold" | "speech" => {
                assert_eq!(ev.source, "silence");
                assert!(block.iter().all(|s| *s == 0.0));
            }
            other => panic!("unexpected segment {other}"),
        }
    }
    let inhale = a.events.iter().find(|e| e.kind == "inhale").unwrap();
    let exhale = a.events.iter().find(|e| e.kind == "exhale").unwrap();
    let db = |e: &synth::RenderedEvent| {
        let i0 = (e.start_s * sr).round() as usize;
        let i1 = (e.end_s * sr).round() as usize;
        20.0 * synth::rms(&a.samples[i0..i1]).log10()
    };
    assert!((db(exhale) - db(inhale) - c.audible.exhale_db_offset).abs() < 1e-6);
    assert!(db(inhale) > c.audible.silence_floor_dbfs);
}

#[test]
fn a_reference_inhale_is_the_speakers_own_samples() {
    let sr = 16_000.0;
    let n = (sr * 1.0) as usize;
    let mut samples = vec![0.0; n];
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f64 / sr;
        if (0.10..0.40).contains(&t) {
            *s = (std::f64::consts::TAU * 80.0 * t).sin();
        }
    }
    let cues = vec![BreathCue {
        start_s: 0.10,
        end_s: 0.40,
        kind: "inhale".into(),
        loudness_dbfs: -40.0,
    }];
    let reference = Reference {
        samples: &samples,
        sample_rate: sr,
        cues: &cues,
    };
    let p = planned("Hello there.", "sympathy", 0.0, 0.5, 0.0);
    let rendered = synth::render_track(&p, sr, Some(&reference), &cfg()).unwrap();
    let ev = rendered.events.iter().find(|e| e.kind == "inhale").unwrap();
    assert_eq!(ev.source, "reference");
    let i0 = (ev.start_s * sr).round() as usize;
    let i1 = (ev.end_s * sr).round() as usize;
    let block = &rendered.samples[i0..i1];
    assert!(
        lag1(block) > 0.95,
        "reference inhale keeps the sine, lag1 {}",
        lag1(block)
    );
    let target = 10f64.powf(ev.loudness_dbfs.unwrap() / 20.0);
    assert!((synth::rms(block) - target).abs() / target < 1e-6);
    let synth_only = synth::render_track(&p, sr, None, &cfg()).unwrap();
    assert_ne!(rendered.samples, synth_only.samples);
}

#[test]
fn splice_inhales_matches_the_features_agent_fallback() {
    let sr = 8_000.0;
    let n = (sr * 1.0) as usize;
    let mut samples = vec![0.0; n];
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f64 / sr;
        if (0.05..0.20).contains(&t) {
            *s = 0.25;
        }
    }
    let cues = vec![BreathCue {
        start_s: 0.05,
        end_s: 0.20,
        kind: "inhale".into(),
        loudness_dbfs: -30.0,
    }];
    let longer = stage::splice_inhales(&samples, sr, &cues, &[0.60]);
    assert!(longer.len() > samples.len());
    let at = (0.60 * sr).round() as usize;
    let inhale_n = ((0.20 - 0.05) * sr).round() as usize;
    assert!(longer[at..at + inhale_n]
        .iter()
        .all(|s| (*s - 0.25).abs() < 1e-9));
    let speech_now = 0.60 + (0.20 - 0.05);
    let again = stage::splice_inhales(&longer, sr, &cues, &[speech_now]);
    assert_eq!(
        again.len(),
        longer.len(),
        "a phrase that already starts with the inhale is left alone"
    );
}

#[test]
fn features_agent_json_parses() {
    let profile = r#"{
        "schema": "galion-voice-decode/2",
        "lungs": {
            "breath_rate_per_min": 12.5,
            "breath_group_s": 1.5,
            "inhale_loudness_dbfs": -42.0,
            "breathiness_cpps_db": null,
            "h1_h2_db": 8.0,
            "n_inhales": 1,
            "voiced_runs_per_s": 1.2,
            "pause_ratio": 0.2,
            "source": "praatfan-voicing-gap",
            "cues": [{"start_s": 0.1, "end_s": 0.4, "kind": "inhale", "loudness_dbfs": -42.0}]
        }
    }"#;
    let v: serde_json::Value = serde_json::from_str(profile).unwrap();
    let lungs = stage::parse_stage(&v).unwrap();
    assert_eq!(lungs.source, "praatfan-voicing-gap");
    assert_eq!(lungs.n_inhales, 1);
    assert!((lungs.inhale_loudness_dbfs.unwrap() - (-42.0)).abs() < 1e-9);
    assert_eq!(lungs.cues[0].kind, "inhale");

    let worker = r#"{"source":"yamnet","duration_s":10.0,"cues":[{"start_s":0.0,"end_s":0.48,"kind":"inhale","loudness_dbfs":0.0}]}"#;
    let w: serde_json::Value = serde_json::from_str(worker).unwrap();
    let lungs = stage::parse_stage(&w).unwrap();
    assert_eq!(lungs.source, "yamnet");
    assert!((lungs.breath_rate_per_min - 6.0).abs() < 1e-9);
}

#[test]
fn mix_puts_phrase_audio_between_the_breaths() {
    let p = planned("Hello there.", "relief", 0.0, 0.5, 0.0);
    let sr = 16_000.0;
    let speech = p.segments.iter().find(|s| s.kind == "speech").unwrap();
    let i0 = (speech.start_s * sr).round() as usize;
    let n = (speech.end_s * sr).round() as usize - i0;
    let tone: Vec<f64> = (0..n).map(|i| (0.2 * (i as f64 * 0.01)).sin()).collect();
    let mixed = synth::mix_phrases(&[tone.clone()], &p, sr, None, &cfg()).unwrap();
    assert!(mixed.samples.len() > tone.len());
    let i0 = (speech.start_s * sr).round() as usize;
    assert_eq!(&mixed.samples[i0..i0 + n], tone.as_slice());
    let inhale = mixed.events.iter().find(|e| e.kind == "inhale").unwrap();
    let a = (inhale.start_s * sr).round() as usize;
    let b = (inhale.end_s * sr).round() as usize;
    assert!(synth::rms(&mixed.samples[a..b]) > 0.0);
}

#[test]
fn wav_roundtrip_stays_outside_the_repo_and_the_cli_plans() {
    let dir = std::env::temp_dir().join(format!("galion-lungs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tone: Vec<f64> = (0..160).map(|i| (i as f64 / 80.0).sin() * 0.5).collect();
    let wav_path = dir.join("tone.wav");
    wav::write_wav_mono_i16(&wav_path, 16_000, &tone).unwrap();
    let back = wav::read_wav(&wav_path).unwrap();
    assert!((back.sample_rate - 16_000.0).abs() < 1e-9);
    assert_eq!(back.samples.len(), tone.len());
    assert!(back.samples.iter().any(|s| s.abs() > 0.2));

    let bin = env!("CARGO_BIN_EXE_galion-lungs");
    let plan_path = dir.join("plan.json");
    let breath_path = dir.join("breaths.wav");
    let status = std::process::Command::new(bin)
        .arg("plan")
        .arg("--text")
        .arg("Hello there.")
        .arg("--category")
        .arg("sadness")
        .arg("--config")
        .arg(galion_lungs::config::bundled_path())
        .arg("--emotions")
        .arg(galion_lungs::inventory::bundled_path())
        .arg("--out")
        .arg(&plan_path)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let body = std::fs::read_to_string(&plan_path).unwrap();
    assert!(body.contains("\"kind\": \"physiology\""));
    assert!(!body.contains("\"category\": \"breath\""));
    let status = std::process::Command::new(bin)
        .arg("render")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--sr")
        .arg("16000")
        .arg("--config")
        .arg(galion_lungs::config::bundled_path())
        .arg("--out")
        .arg(&breath_path)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let rendered = wav::read_wav(&breath_path).unwrap();
    assert!(synth::rms(&rendered.samples) > 0.0);
    assert!(wav_path.starts_with(std::env::temp_dir()));
    let _ = std::fs::remove_dir_all(&dir);
}

fn lag1(x: &[f64]) -> f64 {
    if x.len() < 3 {
        return 0.0;
    }
    let mean = x.iter().sum::<f64>() / x.len() as f64;
    let mut num = 0.0;
    let mut den = 0.0;
    for w in x.windows(2) {
        num += (w[0] - mean) * (w[1] - mean);
    }
    for v in x {
        den += (v - mean) * (v - mean);
    }
    if den < 1e-12 {
        0.0
    } else {
        num / den
    }
}
