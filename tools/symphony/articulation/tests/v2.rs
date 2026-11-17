// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic signals only. No voice audio in git.
use galion_articulation::acoustic;
use galion_articulation::breath;
use galion_articulation::closed_loop::{self, Request};
use galion_articulation::decode::{self, Opts};
use galion_articulation::labels;
use galion_articulation::score;
use galion_articulation::select;
use galion_articulation::state;
use galion_articulation::{Articulation, Section};
use std::collections::BTreeMap;

fn vowel(f0: f64, f1: f64, f2: f64, secs: f64, sr: f64) -> Vec<f64> {
    let n = (secs * sr) as usize;
    let mut y = vec![0.0; n];
    let period = (sr / f0).max(1.0) as usize;
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
    y.iter().map(|v| 0.4 * v / m).collect()
}

fn flat(n: usize, v: f64) -> Vec<Option<f64>> {
    vec![Some(v); n]
}

fn card(f0: f64, f1: f64, f2: f64, f3: f64, vtl: f64) -> Articulation {
    let mut a = Articulation::new(1.6);
    let n = a.n_ticks;
    a.add("f0_hz", Section::Throat, "Hz", "t", flat(n, f0));
    a.add("f0_st", Section::Throat, "st", "t", flat(n, 12.0 * (f0 / 100.0).log2()));
    a.add("hnr_db", Section::Throat, "dB", "t", flat(n, 18.0));
    a.add("voicing", Section::Throat, "fraction", "t", flat(n, 1.0));
    a.add("rms_dbfs", Section::Throat, "dBFS", "t", flat(n, -20.0));
    a.add("f1_hz", Section::Mouth, "Hz", "t", flat(n, f1));
    a.add("f2_hz", Section::Mouth, "Hz", "t", flat(n, f2));
    a.add("f3_hz", Section::Mouth, "Hz", "t", flat(n, f3));
    a.add("vtl_cm", Section::Mouth, "cm", "t", flat(n, vtl));
    a.add("phon_nasal", Section::Nose, "posterior", "t", flat(n, 0.10));
    a.add("ema_tdx", Section::Mouth, "u", "t", flat(n, 0.20));
    a.finish();
    a
}

#[test]
fn speaker_score_puts_male_control_below_same_speaker() {
    let reference = card(180.0, 450.0, 1700.0, 2800.0, 14.5);
    let same = card(182.0, 460.0, 1720.0, 2820.0, 14.6);
    let male = card(110.0, 320.0, 1400.0, 2400.0, 17.5);
    let s_same = score::compare(&reference, &same);
    let s_male = score::compare(&reference, &male);
    assert!(s_same.expression > 0.9, "expression same {}", s_same.expression);
    assert!(s_male.expression > 0.9, "expression male {}", s_male.expression);
    assert!(s_same.speaker > 0.85, "speaker same {}", s_same.speaker);
    assert!(s_male.speaker < 0.55, "speaker male {}", s_male.speaker);
    assert!(s_same.speaker - s_male.speaker > 0.30, "gap {} {}", s_same.speaker, s_male.speaker);
    assert!(s_same.total > s_male.total);
}

#[test]
fn decode_trajectory_is_per_tick_and_maps_to_chatterbox() {
    let sr = 16000.0;
    let x = vowel(200.0, 700.0, 1200.0, 1.6, sr);
    let (mut art, _) = acoustic::analyse(&x, sr);
    art.finish();
    let profile = decode::profile(
        &art,
        &x,
        sr,
        &Opts {
            emotion: Some("seductive"),
            env_name: Some("cafe"),
            emotion_json: None,
            egemaps: None,
            neutral_egemaps: None,
            labels: None,
            cpps_db: None,
            breath_cues: None,
            breath_source: None,
        },
    )
    .unwrap();
    assert_eq!(profile.trajectory.len(), art.n_ticks);
    assert_eq!(profile.target.ticks.len(), art.n_ticks);
    let mid = &profile.target.ticks[art.n_ticks / 2];
    let edge = &profile.target.ticks[0];
    assert!(mid.intensity > edge.intensity);
    let sum: f64 = mid.blend.iter().map(|b| b.w).sum();
    assert!((sum - 1.0).abs() < 1e-6, "blend sum {sum}");
    assert_eq!(mid.blend.len(), 25);
    assert_eq!(mid.burst.len(), 24);
    assert_eq!(profile.target.track, "prosody");
    assert_eq!(profile.target.top, "desire");
    assert_eq!(state::top_blend(&mid.blend), "desire");
    assert!(mid.burst.iter().all(|b| b.w == 0.0));
    assert!(state::PROSODY_RELIABLE.contains(&"desire"));
    assert!(!state::PROSODY_RELIABLE.contains(&"triumph"));
    assert!(state::COWEN.contains(&"triumph"));
    let laugh = state::target_trajectory("laughter", 4).unwrap();
    assert_eq!(state::top_blend(&laugh[2].blend), "neutral");
    assert_eq!(state::top_blend(&laugh[2].burst), "amusement");
    assert!(state::target_trajectory("breath", 4).is_err());
    assert!(state::target_trajectory("triumph", 4).is_ok());
    assert!(state::resolve("sigh").is_err());
    assert!(profile.chatterbox.exaggeration > 0.3 && profile.chatterbox.exaggeration <= 1.5);
    assert!(profile.chatterbox.cfg >= 0.3 && profile.chatterbox.cfg <= 0.85);
    assert!(profile.chatterbox.speed >= 0.8 && profile.chatterbox.speed <= 1.25);
    assert_eq!(profile.environment.effects.room, "cafe");
    assert!(profile.environment.effects.lombard);
    assert!(profile.speaker.f0_hz > 150.0);
    assert_ne!(profile.speaker.f0_hz, profile.target.ticks[0].arousal);
    assert!(profile.gaps.iter().any(|g| g.contains("a1_p0")));
    assert!(profile.gaps.iter().any(|g| g.contains("cpps")));
}

#[test]
fn loop_raises_exaggeration_until_level_matches() {
    let sr = 16000.0;
    let x = vowel(200.0, 700.0, 1200.0, 1.6, sr);
    let report = closed_loop::run_standin(
        &x,
        sr,
        &Request {
            emotion: Some("interest"),
            env_name: Some("studio"),
            enrol: None,
            max_iter: 3,
            start_exaggeration: Some(0.2),
        },
    )
    .unwrap();
    assert!(report.applied.len() >= 2, "iters {}", report.applied.len());
    assert!(report.applied.last().unwrap().exaggeration > report.applied[0].exaggeration);
    assert!(report.passed, "deltas {:?}", report.deltas);
    assert_eq!(report.renderer, "stand-in");
    assert!(report.deltas.iter().any(|d| d.field == "f0_hz" && d.corrects.is_none() && d.pass));
}

#[test]
fn select_keeps_enrolment_speaker_and_drops_the_other() {
    let sr = 16000.0;
    let enrol = vowel(200.0, 700.0, 1200.0, 1.0, sr);
    let mut scene = vowel(200.0, 700.0, 1200.0, 0.8, sr);
    scene.extend(std::iter::repeat(0.0).take((0.25 * sr) as usize));
    scene.extend(vowel(110.0, 300.0, 2300.0, 0.8, sr));
    scene.extend(std::iter::repeat(0.0).take((0.25 * sr) as usize));
    let mut s = 0x1234_5678u32;
    for _ in 0..(0.5 * sr) as usize {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        scene.push(0.2 * ((s >> 8) as f64 / 16_777_216.0 - 0.5));
    }
    scene.extend(vowel(200.0, 710.0, 1210.0, 0.8, sr));
    let table = select::select(&scene, sr, &enrol, None).unwrap();
    let kept: Vec<_> = table.segments.iter().filter(|s| s.kept).collect();
    assert!(kept.len() >= 2, "kept {kept:?} all {:?}", table.segments);
    assert!(table.segments.iter().any(|s| !s.kept && s.speaker != "target"));
    let audio = select::concat_kept(&scene, sr, &table);
    assert!(audio.len() < scene.len());
    assert!(audio.len() > (sr as usize));
}

#[test]
fn breath_cue_on_a_gap_between_vowels() {
    let sr = 16000.0;
    let mut x = vec![0.0; (0.4 * sr) as usize];
    let mut s = 7u32;
    for _ in 0..(0.24 * sr) as usize {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        x.push(0.03 * ((s >> 8) as f64 / 16_777_216.0 - 0.5));
    }
    x.extend(vowel(180.0, 500.0, 1500.0, 0.8, sr));
    let (mut art, _) = acoustic::analyse(&x, sr);
    art.finish();
    let lungs = breath::measure(&art, None);
    assert!(lungs.n_inhales >= 1, "cues {:?}", lungs.cues);
    assert!(lungs.inhale_loudness_dbfs.unwrap() < -20.0);
    assert!(lungs.breathiness_cpps_db.is_none());
    let longer = breath::splice_inhales(&x, sr, &lungs.cues, &[1.2]);
    assert!(longer.len() > x.len());
}

#[test]
fn labels_file_scores_a_matching_z() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/emotion-labels-decoder.toml");
    let labels = labels::load(&path).unwrap();
    let mut z = BTreeMap::new();
    z.insert("F0semitoneFrom27.5Hz_sma3nz_amean".into(), 0.8);
    z.insert("loudness_sma3_amean".into(), 0.6);
    z.insert("HNRdBACF_sma3nz_amean".into(), 0.2);
    assert_eq!(labels.alias.get("seductive").map(String::as_str), Some("desire"));
    assert_eq!(labels.set.len(), 24);
    assert!(labels.set.contains(&"triumph".into()));
    assert!(labels.set.contains(&"desire".into()));
    assert!(!labels.set.iter().any(|s| s == "neutral" || s == "seductive" || s == "breath"));
    let inventory = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../configs/emotions-v2.toml"),
    )
    .unwrap();
    for name in state::COWEN {
        assert!(inventory.contains(&format!("\"{name}\"")), "{name} missing from emotions-v2.toml");
    }
    let fixture = r#"
schema = 2
relative_to = "speaker_neutral"
[features."F0semitoneFrom27.5Hz_sma3nz_amean"]
unit = "z"
neutral_std = 2.0
[emotions.desire.female]
"F0semitoneFrom27.5Hz_sma3nz_amean" = 0.8
[emotions.anger.female]
"F0semitoneFrom27.5Hz_sma3nz_amean" = -0.4
"#;
    let labels = labels::parse(fixture).unwrap();
    let m = labels::match_emotions(&z, &labels, "female");
    assert!(m["desire"] > 0.95, "{}", m["desire"]);
    assert!(m["desire"] > m["anger"]);
    assert!(m["anger"] > 0.0 && m["anger"] <= 1.0);
}
