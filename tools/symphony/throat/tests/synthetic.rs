// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic checks. No audio file is an input. Nothing here is a measurement
//! of a person.

use std::fs;
use std::path::PathBuf;

use galion_throat::config::{self, Config};
use galion_throat::contract::{self, FeelingDrive};
use galion_throat::inventory;
use galion_throat::lungs_in::LungsTick;
use galion_throat::render::{self, Request, Utterance};
use galion_throat::TICK_MS;
use serde_json::json;

fn shipped() -> Config {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/throat.toml");
    config::load_path(&path).unwrap()
}

fn speech(t_ms: u32, f0_st: Option<f64>, loudness_db: Option<f64>) -> LungsTick {
    LungsTick {
        t_ms,
        subglottal_pressure_rel: Some(1.0),
        f0_st,
        loudness_db,
        segment: "speech".into(),
    }
}

fn gap(t_ms: u32, segment: &str) -> LungsTick {
    LungsTick {
        t_ms,
        subglottal_pressure_rel: None,
        f0_st: None,
        loudness_db: None,
        segment: segment.into(),
    }
}

fn run(
    feeling: serde_json::Value,
    lungs: Vec<LungsTick>,
    cfg: &Config,
    f0: f64,
    rd: Option<f64>,
    source: bool,
) -> (Utterance, Option<Vec<f32>>) {
    let drive: FeelingDrive = contract::parse_feeling_drive(&feeling).unwrap();
    let req = Request {
        speaker: contract::parse_speaker(f0, rd).unwrap(),
        feeling: drive,
        lungs,
    };
    render::render(&req, cfg, source).unwrap()
}

fn peaks(x: &[f32]) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 1..x.len().saturating_sub(1) {
        if x[i] < -0.2 && x[i] < x[i - 1] && x[i] <= x[i + 1] {
            if out.last().is_none_or(|p| i - p > 20) {
                out.push(i);
            }
        }
    }
    out
}

fn median_period(marks: &[usize]) -> f64 {
    let mut gaps: Vec<f64> = marks.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    gaps[gaps.len() / 2]
}

#[test]
fn shipped_scales_stay_unknown() {
    let cfg = shipped();
    assert_eq!(cfg.tick_ms, TICK_MS);
    assert!((cfg.rd_baseline - 1.0).abs() < 1e-9);
    assert!(cfg.ks.is_none());
    for key in ["pitch_st", "pitch_range_st", "rd_db", "jitter", "shimmer"] {
        assert!(cfg.scale(None, key).is_none(), "{key}");
    }
    let (utt, src) = run(
        json!({}),
        vec![speech(0, Some(0.0), Some(0.0))],
        &cfg,
        180.0,
        None,
        false,
    );
    assert!(src.is_none());
    assert!(!utt.reference.generated);
    let t = &utt.ticks[0];
    assert!(t.voiced);
    assert_eq!(t.phonation, "modal");
    assert_eq!(t.pitch_sign, "baseline");
    assert!(t.f0_st_from_feeling.is_none());
    assert!((t.f0_st_vs_baseline.unwrap() - 0.0).abs() < 1e-9);
    assert!((t.open_quotient.unwrap() - 0.65).abs() < 0.01);
    assert!(t.jitter.is_none() && t.shimmer.is_none());
    assert!(utt.patch.jitter.is_none());
    assert_eq!(utt.schema, "galion-throat/1");
    assert_eq!(utt.patch.phonation, vec!["modal".to_string()]);
}

#[test]
fn lungs_offset_is_the_contour_until_a_feeling_step_exists() {
    let cfg = shipped();
    let lungs = vec![
        speech(0, Some(1.5), Some(0.0)),
        speech(80, Some(0.0), Some(0.0)),
        speech(160, Some(-1.25), Some(0.0)),
    ];
    let (utt, _) = run(
        json!({"category": "sadness", "intensity": 1.0}),
        lungs,
        &cfg,
        180.0,
        None,
        false,
    );
    let got: Vec<f64> = utt
        .ticks
        .iter()
        .map(|t| t.f0_st_vs_baseline.unwrap())
        .collect();
    assert_eq!(got, vec![1.5, 0.0, -1.25]);
    assert!(utt.ticks.iter().all(|t| t.f0_st_from_feeling.is_none()));
    assert!(utt.ticks.iter().all(|t| t.pitch_sign == "lower"));
    assert!(utt.ticks.iter().all(|t| !t.feeling_pitch_applied));
}

#[test]
fn feeling_pitch_follows_the_published_sign_only_when_configured() {
    let mut cfg = shipped();
    cfg.scales.insert("pitch_st".into(), Some(2.0));
    cfg.scales.insert("pitch_range_st".into(), Some(1.0));
    let lungs = vec![speech(0, Some(0.5), Some(0.0))];
    let (sad, _) = run(
        json!({"category": "sadness", "dominance": 0.0}),
        lungs.clone(),
        &cfg,
        200.0,
        None,
        false,
    );
    assert!((sad.ticks[0].f0_st_from_feeling.unwrap() - (-2.0)).abs() < 1e-9);
    assert!((sad.ticks[0].f0_st_vs_baseline.unwrap() - (-1.5)).abs() < 1e-9);
    assert!((sad.ticks[0].f0_range_st.unwrap() - (-1.0)).abs() < 1e-9);
    let (hot, _) = run(
        json!({"category": "anger", "dominance": 0.4}),
        lungs.clone(),
        &cfg,
        200.0,
        None,
        false,
    );
    assert_eq!(hot.ticks[0].pole.as_deref(), Some("hot_anger"));
    assert!((hot.ticks[0].f0_st_from_feeling.unwrap() - 2.0).abs() < 1e-9);
    assert_eq!(hot.ticks[0].prosody_tier, "reliable");
    let (panic, _) = run(
        json!({"category": "fear", "dominance": 1.0}),
        lungs.clone(),
        &cfg,
        200.0,
        None,
        false,
    );
    assert_eq!(panic.ticks[0].pole.as_deref(), Some("panic"));
    assert!(panic.ticks[0].f0_st_from_feeling.unwrap() > 0.0);
    assert!(
        panic.ticks[0].f0_range_st.is_none(),
        "fear F0 range is unresolved"
    );
    let (anx, _) = run(
        json!({"category": "fear", "dominance": -0.2}),
        lungs.clone(),
        &cfg,
        200.0,
        None,
        false,
    );
    assert_eq!(anx.ticks[0].pole.as_deref(), Some("anxiety"));
    assert!(anx.ticks[0].f0_st_from_feeling.unwrap() < 0.0);
    let (awe, _) = run(
        json!({"category": "awe"}),
        lungs.clone(),
        &cfg,
        200.0,
        None,
        false,
    );
    assert!(awe.ticks[0].f0_st_from_feeling.is_none());
    assert_eq!(awe.ticks[0].pitch_sign, "unknown");
    assert!((awe.ticks[0].f0_st_vs_baseline.unwrap() - 0.5).abs() < 1e-9);
    let (emb, _) = run(
        json!({"category": "embarrassment"}),
        lungs,
        &cfg,
        200.0,
        None,
        false,
    );
    assert_eq!(emb.ticks[0].prosody_tier, "extended");
    assert!(emb.ticks[0].f0_st_from_feeling.is_none());
}

#[test]
fn pole_step_overrides_the_shared_step() {
    let mut cfg = shipped();
    cfg.scales.insert("pitch_st".into(), Some(2.0));
    let mut hot = std::collections::BTreeMap::new();
    hot.insert("pitch_st".into(), Some(5.0));
    cfg.poles.insert("hot_anger".into(), hot);
    let lungs = vec![speech(0, None, None)];
    let (up, _) = run(
        json!({"category": "anger", "dominance": 1.0}),
        lungs.clone(),
        &cfg,
        150.0,
        None,
        false,
    );
    let (down, _) = run(
        json!({"category": "anger", "dominance": -1.0}),
        lungs,
        &cfg,
        150.0,
        None,
        false,
    );
    assert!((up.ticks[0].f0_st_from_feeling.unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(down.ticks[0].pole.as_deref(), Some("cold_anger"));
    assert!((down.ticks[0].f0_st_from_feeling.unwrap() - 2.0).abs() < 1e-9);
}

#[test]
fn burst_and_half_intensity_do_not_invent_a_speech_shift() {
    let mut cfg = shipped();
    cfg.scales.insert("pitch_st".into(), Some(4.0));
    let lungs = vec![speech(0, Some(0.25), Some(0.0))];
    let (burst, _) = run(
        json!({"category": "anger", "track": "burst", "dominance": 1.0}),
        lungs.clone(),
        &cfg,
        180.0,
        None,
        false,
    );
    assert_eq!(burst.ticks[0].pitch_sign, "burst");
    assert!(burst.ticks[0].f0_st_from_feeling.is_none());
    assert!((burst.ticks[0].f0_st_vs_baseline.unwrap() - 0.25).abs() < 1e-9);
    let (half, _) = run(
        json!({"category": "elation", "intensity": 0.5}),
        lungs,
        &cfg,
        180.0,
        None,
        false,
    );
    assert!((half.ticks[0].f0_st_from_feeling.unwrap() - 2.0).abs() < 1e-9);
}

#[test]
fn high_frequency_energy_moves_rd_only_when_configured() {
    let cfg = shipped();
    let lungs = vec![speech(0, Some(0.0), Some(0.0))];
    let (plain, _) = run(
        json!({"category": "anger", "dominance": 1.0}),
        lungs.clone(),
        &cfg,
        180.0,
        None,
        false,
    );
    assert!((plain.ticks[0].rd.unwrap() - 1.0).abs() < 1e-6);
    assert!(!plain.ticks[0].feeling_rd_applied);
    let mut cfg = shipped();
    cfg.scales.insert("rd_db".into(), Some(6.0));
    let (anger, _) = run(
        json!({"category": "anger", "dominance": 1.0}),
        lungs.clone(),
        &cfg,
        180.0,
        None,
        false,
    );
    let (sad, _) = run(
        json!({"category": "sadness"}),
        lungs.clone(),
        &cfg,
        180.0,
        None,
        false,
    );
    let (fear, _) = run(
        json!({"category": "fear", "dominance": 1.0}),
        lungs,
        &cfg,
        180.0,
        None,
        false,
    );
    assert!(anger.ticks[0].rd.unwrap() < 1.0);
    assert!(sad.ticks[0].rd.unwrap() > 1.0);
    assert!(anger.ticks[0].open_quotient.unwrap() < sad.ticks[0].open_quotient.unwrap());
    assert!(
        anger.ticks[0].spectral_tilt_db_oct.unwrap() > sad.ticks[0].spectral_tilt_db_oct.unwrap()
    );
    assert!(anger.ticks[0].fa_hz.unwrap() > sad.ticks[0].fa_hz.unwrap());
    assert!(
        (fear.ticks[0].rd.unwrap() - 1.0).abs() < 1e-6,
        "fear HF energy is unresolved"
    );
    assert_eq!(fear.ticks[0].hf_sign, "unresolved");
}

#[test]
fn effort_ks_lowers_rd_when_lungs_loudness_rises() {
    let mut cfg = shipped();
    cfg.ks = Some(0.5);
    let quiet = vec![speech(0, Some(0.0), Some(0.0))];
    let loud = vec![speech(0, Some(0.0), Some(6.0))];
    let (a, _) = run(json!({}), quiet, &cfg, 180.0, None, false);
    let (b, _) = run(json!({}), loud, &cfg, 180.0, None, false);
    assert!(!a.ticks[0].effort_applied);
    assert!(b.ticks[0].effort_applied);
    assert!(b.ticks[0].rd.unwrap() < a.ticks[0].rd.unwrap());
    assert!((b.ticks[0].ee_rel.unwrap() - 10_f64.powf(6.0 / 20.0)).abs() < 1e-9);
    assert_eq!(b.ticks[0].ee_source.as_deref(), Some("lungs_loudness_db"));
    let mut off = shipped();
    off.ks = None;
    let (c, _) = run(
        json!({}),
        vec![speech(0, Some(0.0), Some(6.0))],
        &off,
        180.0,
        None,
        false,
    );
    assert!(!c.ticks[0].effort_applied);
    assert!((c.ticks[0].rd.unwrap() - 1.0).abs() < 1e-6);
}

#[test]
fn unvoiced_lungs_segments_do_not_set_a_pitch_target() {
    let mut cfg = shipped();
    cfg.scales.insert("pitch_st".into(), Some(3.0));
    let lungs = vec![
        gap(0, "inhale"),
        gap(80, "hold"),
        speech(160, Some(2.0), Some(0.0)),
        gap(240, "exhale"),
    ];
    let feeling = json!({"category": "anger", "dominance": 1.0});
    let (utt, src) = run(feeling, lungs, &cfg, 100.0, None, true);
    assert_eq!(utt.ticks[0].phonation, "unvoiced");
    assert!(utt.ticks[0].f0_hz.is_none());
    assert!(utt.ticks[0].open_quotient.is_none());
    assert_eq!(utt.ticks[1].phonation, "unvoiced");
    assert_eq!(utt.ticks[3].phonation, "unvoiced");
    assert!(utt.ticks[2].voiced);
    assert!(utt.ticks[2].f0_st_from_feeling.unwrap() > 0.0);
    let hop = 16000 * 80 / 1000;
    let src = src.unwrap();
    assert_eq!(src.len(), hop * 4);
    assert!(src[..hop].iter().all(|s| *s == 0.0));
    assert!(src[hop..2 * hop].iter().all(|s| *s == 0.0));
    assert!(src[3 * hop..].iter().all(|s| *s == 0.0));
    assert!(src[2 * hop..3 * hop].iter().any(|s| s.abs() > 0.2));
}

#[test]
fn speech_without_pressure_is_unvoiced() {
    let cfg = shipped();
    let tick = LungsTick {
        t_ms: 0,
        subglottal_pressure_rel: None,
        f0_st: Some(3.0),
        loudness_db: Some(2.0),
        segment: "speech".into(),
    };
    let (utt, _) = run(
        json!({"category": "elation"}),
        vec![tick],
        &cfg,
        180.0,
        None,
        false,
    );
    assert!(!utt.ticks[0].voiced);
    assert!(utt.ticks[0].f0_hz.is_none());
}

#[test]
fn phonation_labels_follow_rd_anchors_and_an_explicit_creak() {
    let cfg = shipped();
    let lungs = vec![speech(0, Some(0.0), Some(0.0))];
    let (pressed, _) = run(json!({}), lungs.clone(), &cfg, 110.0, Some(0.5), false);
    let (breathy, _) = run(json!({}), lungs.clone(), &cfg, 110.0, Some(2.7), false);
    let (creak, _) = run(
        json!({"phonation": "creaky"}),
        lungs,
        &cfg,
        110.0,
        None,
        false,
    );
    assert_eq!(pressed.ticks[0].phonation, "pressed");
    assert!(pressed.ticks[0].open_quotient.unwrap() < 0.5);
    assert_eq!(breathy.ticks[0].phonation, "breathy");
    assert!(breathy.ticks[0].open_quotient.unwrap() > 0.7);
    assert!(
        breathy.ticks[0].spectral_tilt_db_oct.unwrap()
            < pressed.ticks[0].spectral_tilt_db_oct.unwrap()
    );
    assert_eq!(creak.ticks[0].phonation, "creaky");
    assert!((creak.ticks[0].rd.unwrap() - 1.0).abs() < 1e-6);
    assert!(creak.ticks[0].jitter.is_none());
}

#[test]
fn reference_period_tracks_f0_and_jitter_is_deterministic() {
    let cfg = shipped();
    let flat = vec![
        speech(0, Some(0.0), Some(0.0)),
        speech(80, Some(0.0), Some(0.0)),
        speech(160, Some(0.0), Some(0.0)),
        speech(240, Some(0.0), Some(0.0)),
    ];
    let (_, a) = run(json!({}), flat.clone(), &cfg, 100.0, None, true);
    let (_, b) = run(json!({}), flat.clone(), &cfg, 100.0, None, true);
    let a = a.unwrap();
    assert_eq!(a, b.unwrap());
    let period = median_period(&peaks(&a));
    assert!((period - 160.0).abs() <= 1.0, "period {period}");
    let raised = vec![
        speech(0, Some(7.0), Some(0.0)),
        speech(80, Some(7.0), Some(0.0)),
        speech(160, Some(7.0), Some(0.0)),
        speech(240, Some(7.0), Some(0.0)),
    ];
    let (_, up) = run(json!({}), raised, &cfg, 100.0, None, true);
    let up_period = median_period(&peaks(&up.unwrap()));
    assert!(up_period < period - 20.0, "raised {up_period} vs {period}");

    let mut jig = shipped();
    jig.scales.insert("jitter".into(), Some(0.08));
    jig.scales.insert("shimmer".into(), Some(0.2));
    let (_, wavy) = run(json!({}), flat, &jig, 100.0, None, true);
    let wavy = wavy.unwrap();
    let marks = peaks(&wavy);
    let mut gaps: Vec<f64> = marks.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(gaps.last().unwrap() - gaps.first().unwrap() > 4.0);
    let mut depths: Vec<f32> = marks.iter().map(|&i| wavy[i].abs()).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(depths.last().unwrap() - depths.first().unwrap() > 0.05);
}

#[test]
fn moments_change_on_the_80_ms_clock() {
    let mut cfg = shipped();
    cfg.scales.insert("pitch_st".into(), Some(2.0));
    let lungs = vec![
        speech(0, Some(0.0), Some(0.0)),
        speech(80, Some(0.0), Some(0.0)),
        speech(160, Some(0.0), Some(0.0)),
    ];
    let feeling = json!({
        "moments": [
            {"t_ms": 0, "category": "sadness"},
            {"t_ms": 160, "category": "elation"}
        ]
    });
    let (utt, _) = run(feeling, lungs, &cfg, 160.0, None, false);
    assert!(utt.ticks[0].f0_st_from_feeling.unwrap() < 0.0);
    assert!(utt.ticks[1].f0_st_from_feeling.unwrap() < 0.0);
    assert!(utt.ticks[2].f0_st_from_feeling.unwrap() > 0.0);
    assert_eq!(utt.patch.t_ms, vec![0, 80, 160]);
}

#[test]
fn names_and_bad_clocks_are_rejected() {
    let cfg = shipped();
    let lungs = vec![speech(0, Some(0.0), Some(0.0))];
    let req = |feeling: serde_json::Value| {
        let drive = contract::parse_feeling_drive(&feeling)?;
        render::render(
            &Request {
                speaker: contract::parse_speaker(180.0, None)?,
                feeling: drive,
                lungs: lungs.clone(),
            },
            &cfg,
            false,
        )
    };
    assert!(req(json!({"category": "happy"})).is_err());
    assert!(req(json!({"category": "breath"})).is_err());
    assert!(req(json!({"category": "nope"})).is_err());
    assert!(req(json!({"phonation": "breathy"})).is_err());
    assert!(contract::parse_speaker(0.0, None).is_err());
    let laughed = contract::parse_feeling_drive(&json!({"category": "laughter"})).unwrap();
    match laughed {
        FeelingDrive::Constant(f) => {
            assert_eq!(f.category.unwrap().as_str(), "amusement");
            assert_eq!(f.track.as_str(), "burst");
        }
        _ => panic!("laughter is one state"),
    }
    assert_eq!(
        inventory::resolve("disgusted")
            .unwrap()
            .category
            .unwrap()
            .as_str(),
        "contempt"
    );
    assert!(inventory::resolve("neutral").unwrap().category.is_none());
    let bad = json!({"ticks": [{"t_ms": 0, "segment": "speech", "subglottal_pressure_rel": 1.0}, {"t_ms": 40, "segment": "speech", "subglottal_pressure_rel": 1.0}]});
    assert!(galion_throat::lungs_in::parse_lungs(&bad).is_err());
}

#[test]
fn cli_writes_json_and_a_synthetic_wav_outside_the_repo() {
    let dir = std::env::temp_dir().join(format!("galion-throat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let feeling = dir.join("feeling.json");
    let lungs = dir.join("lungs.json");
    let out = dir.join("controls.json");
    let wav = dir.join("source.wav");
    fs::write(&feeling, r#"{"category":"sadness"}"#).unwrap();
    fs::write(
        &lungs,
        r#"{"schema":"galion-lungs/1","ticks":[{"t_ms":0,"segment":"speech","subglottal_pressure_rel":1.0,"f0_st":-0.4,"loudness_db":0.0}]}"#,
    )
    .unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_galion-throat"));
    let cfg =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/throat.toml");
    let status = std::process::Command::new(bin)
        .args([
            "render",
            "--feeling",
            feeling.to_str().unwrap(),
            "--lungs",
            lungs.to_str().unwrap(),
            "--baseline-hz",
            "175",
            "--config",
            cfg.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--source",
            wav.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(value["schema"], "galion-throat/1");
    assert_eq!(value["patch"]["pitch_st"][0], json!(-0.4));
    assert_eq!(value["reference"]["kind"], "lf_flow_derivative");
    assert_eq!(value["reference"]["embedded"], false);
    let header = fs::read(&wav).unwrap();
    assert_eq!(&header[..4], b"RIFF");
    assert!(header.len() > 44);
    fs::remove_dir_all(&dir).unwrap();
}
