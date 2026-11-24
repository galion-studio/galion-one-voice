// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Gate tests. Every signal is synthesized in this file or in `synth`.
//! No recording, no embedding, no weight.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use galion_preclone::synth;
use galion_preclone::{
    analyse, estimate_rt60_s, estimate_snr_db, parse_moments, profile_bytes, read_wav, MomentAxis,
    MomentFrame, Request, Segment, SegmentKind, SCHEMA, STAGE,
};

const SR: u32 = 16_000;

fn req<'a>(samples: &'a [f32], target: Option<f64>, moments: &'a [MomentFrame]) -> Request<'a> {
    Request {
        samples,
        sample_rate_hz: SR,
        target_f0_hz: target,
        moments,
    }
}

fn overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

fn seconds(seg: &Segment) -> (f64, f64) {
    (
        seg.start_sample as f64 / f64::from(SR),
        seg.end_sample as f64 / f64::from(SR),
    )
}

fn kind_overlap(segs: &[Segment], kind: SegmentKind, region: (f64, f64)) -> f64 {
    segs.iter()
        .filter(|s| s.kind == kind)
        .map(|s| {
            let (a, b) = seconds(s);
            overlap(a, b, region.0, region.1)
        })
        .sum()
}

fn kept_against(segs: &[Segment], truth: (f64, f64)) -> (f64, f64) {
    let mut kept = 0.0;
    let mut hit = 0.0;
    for s in segs {
        if !s.kept {
            continue;
        }
        let (a, b) = seconds(s);
        kept += b - a;
        hit += overlap(a, b, truth.0, truth.1);
    }
    let precision = if kept > 0.0 { hit / kept } else { 0.0 };
    let recall = hit / (truth.1 - truth.0);
    (precision, recall)
}

fn dump(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| {
            let (a, b) = seconds(s);
            format!(
                "{:?} {:.3}-{:.3} kept={} f0={:.1} snr={:.1}",
                s.kind, a, b, s.kept, s.f0_hz, s.snr_db
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

struct Take {
    samples: Vec<f32>,
    target: (f64, f64),
    other: (f64, f64),
    music: (f64, f64),
}

/// Two synthetic speakers, a noise bed, and a chord. Regions are the truth.
fn messy() -> Take {
    let n = SR as usize * 43 / 10;
    let mut x = synth::white(n, 7);
    synth::scale_rms(&mut x, 0.008);
    let a = synth::formant_voice(
        SR as usize,
        SR,
        140.0,
        0.08,
        &[(700.0, 90.0), (1200.0, 110.0), (2600.0, 150.0)],
    );
    let b = synth::formant_voice(
        SR as usize,
        SR,
        230.0,
        0.08,
        &[(400.0, 70.0), (1800.0, 120.0), (2800.0, 160.0)],
    );
    let music = synth::chord(SR as usize * 8 / 10, SR, &[196.0, 262.0, 330.0], 0.08);
    let fade = SR as usize / 100;
    synth::place(&mut x, &a, SR as usize * 4 / 10, fade);
    synth::place(&mut x, &b, SR as usize * 18 / 10, fade);
    synth::place(&mut x, &music, SR as usize * 31 / 10, fade);
    Take {
        samples: x,
        target: (0.40, 1.40),
        other: (1.80, 2.80),
        music: (3.10, 3.90),
    }
}

#[test]
fn snr_on_white_and_pink_mixes_stays_inside_tolerance() {
    for (snr, seed) in [(10.0, 11u64), (20.0, 12), (30.0, 13)] {
        let x = synth::noise_mix(SR, snr, false, seed);
        let est = estimate_snr_db(&x, SR).expect("snr");
        assert!(
            (est - snr).abs() < 1.5,
            "white mix {snr} dB estimated {est} dB"
        );
        let profile = analyse(&req(&x, Some(140.0), &[])).unwrap();
        let filed = profile.environment.snr_db.expect("profile snr");
        assert!((filed - est).abs() < 1e-6);
    }
    let pink = synth::noise_mix(SR, 15.0, true, 19);
    let est = estimate_snr_db(&pink, SR).expect("pink snr");
    assert!(
        (est - 15.0).abs() < 2.0,
        "pink mix 15 dB estimated {est} dB, tilt {:?}",
        analyse(&req(&pink, Some(140.0), &[]))
            .unwrap()
            .environment
            .noise_tilt_db_per_octave
    );
}

#[test]
fn noise_colour_separates_white_from_pink() {
    let white = synth::noise_mix(SR, 20.0, false, 3);
    let pink = synth::noise_mix(SR, 20.0, true, 5);
    let w = analyse(&req(&white, Some(140.0), &[])).unwrap();
    let p = analyse(&req(&pink, Some(140.0), &[])).unwrap();
    assert_eq!(
        w.environment.noise_color, "white",
        "tilt {:?}",
        w.environment.noise_tilt_db_per_octave
    );
    assert_eq!(
        p.environment.noise_color, "pink",
        "tilt {:?}",
        p.environment.noise_tilt_db_per_octave
    );
}

#[test]
fn rt60_on_a_synthetic_tail_stays_inside_tolerance() {
    for rt in [0.30_f64, 0.60] {
        let x = synth::room_tail(SR, rt, 21);
        let est = estimate_rt60_s(&x, SR).unwrap_or_else(|e| panic!("rt60 {rt}: {e}"));
        assert!((est - rt).abs() <= 0.05, "RT60 {rt} s estimated {est} s");
        let profile = analyse(&req(&x, None, &[])).unwrap();
        let filed = profile.environment.rt60_s.expect("profile rt60");
        assert!((filed - est).abs() < 1e-9);
        assert!(profile.environment.rt60_note.is_empty());
    }
}

#[test]
fn two_speakers_keep_the_target_and_flag_the_music_bed() {
    let take = messy();
    let profile = analyse(&req(&take.samples, Some(140.0), &[])).unwrap();
    let (precision, recall) = kept_against(&profile.segments, take.target);
    // Precision: kept time that lies inside the target interval, over kept time.
    // Recall: that same overlap, over the target interval.
    assert!(
        precision >= 0.90,
        "precision {precision:.3} recall {recall:.3} {}",
        dump(&profile.segments)
    );
    assert!(
        recall >= 0.80,
        "precision {precision:.3} recall {recall:.3} {}",
        dump(&profile.segments)
    );
    let other_hit = kind_overlap(&profile.segments, SegmentKind::Other, take.other);
    assert!(
        other_hit >= 0.60,
        "other-speaker overlap {other_hit:.3} {}",
        dump(&profile.segments)
    );
    let kept_on_other = profile
        .segments
        .iter()
        .filter(|s| s.kept)
        .map(|s| {
            let (a, b) = seconds(s);
            overlap(a, b, take.other.0, take.other.1)
        })
        .sum::<f64>();
    assert!(
        kept_on_other < 0.05,
        "kept leaked into the other speaker by {kept_on_other:.3}s"
    );
    let music_hit = kind_overlap(&profile.segments, SegmentKind::Music, take.music);
    assert!(
        music_hit >= 0.50,
        "music overlap {music_hit:.3} {}",
        dump(&profile.segments)
    );
    assert!(profile.environment.music_bed);
    assert!(profile.voice.kept_samples > SR as usize / 2);
    assert!(
        (profile.voice.f0_hz - 140.0).abs() < 5.0,
        "target f0 {}",
        profile.voice.f0_hz
    );
}

#[test]
fn clean_formant_baseline_recovers_f0_and_formants() {
    let x = synth::formant_voice(
        SR as usize,
        SR,
        140.0,
        0.05,
        &[(500.0, 80.0), (1500.0, 110.0), (2500.0, 140.0)],
    );
    let profile = analyse(&req(&x, Some(140.0), &[])).unwrap();
    assert!(
        (profile.voice.f0_hz - 140.0).abs() < 4.0,
        "f0 {} kept {} segs {}",
        profile.voice.f0_hz,
        profile.voice.kept_samples,
        dump(&profile.segments)
    );
    for (got, want) in [
        (profile.voice.f1_hz, 500.0),
        (profile.voice.f2_hz, 1500.0),
        (profile.voice.f3_hz, 2500.0),
    ] {
        assert!((got - want).abs() <= 80.0, "formant {got} wanted {want}");
    }
    assert!(profile.voice.voiced_fraction > 0.8);
    assert!(!profile.emotion.attached);
    assert_eq!(profile.emotion.frames.len(), 0);
}

#[test]
fn profile_bytes_are_deterministic_and_name_the_stage() {
    let x = messy().samples;
    let moments = [
        MomentFrame {
            t_ms: 80,
            axes: vec![MomentAxis {
                name: "valence".into(),
                value: 0.25,
            }],
        },
        MomentFrame {
            t_ms: 0,
            axes: vec![
                MomentAxis {
                    name: "arousal".into(),
                    value: -0.5,
                },
                MomentAxis {
                    name: "valence".into(),
                    value: 0.1,
                },
            ],
        },
    ];
    let swapped = [
        MomentFrame {
            t_ms: 0,
            axes: vec![
                MomentAxis {
                    name: "valence".into(),
                    value: 0.1,
                },
                MomentAxis {
                    name: "arousal".into(),
                    value: -0.5,
                },
            ],
        },
        MomentFrame {
            t_ms: 80,
            axes: vec![MomentAxis {
                name: "valence".into(),
                value: 0.25,
            }],
        },
    ];
    let a = profile_bytes(&analyse(&req(&x, Some(140.0), &moments)).unwrap());
    let b = profile_bytes(&analyse(&req(&x, Some(140.0), &moments)).unwrap());
    let c = profile_bytes(&analyse(&req(&x, Some(140.0), &swapped)).unwrap());
    assert_eq!(a, b);
    assert_eq!(a, c);
    let text = String::from_utf8(a.clone()).unwrap();
    assert!(text.contains(&format!("\"schema\": \"{SCHEMA}\"")));
    assert!(text.contains(&format!("\"stage\": \"{STAGE}\"")));
    assert!(text.contains("\"compute\": \"cpu\""));
    assert!(text.contains("\"hook\": \"galion-feeling\""));
    assert!(text.contains("\"tick_ms\": 80"));
    assert!(text.contains("\"kind\": \"steady-baseline\""));
    assert!(text.contains("\"role\": \"baseline-separate-from-moment\""));
    let parsed: serde_json::Value = serde_json::from_slice(&a).unwrap();
    assert_eq!(parsed["emotion"]["moment"]["frame_count"], 2);
    assert_eq!(parsed["emotion"]["moment"]["frames"][0]["t_ms"], 0);
    assert_eq!(
        parsed["emotion"]["moment"]["frames"][0]["axes"][0]["name"],
        "arousal"
    );
    let bare = analyse(&req(&x, Some(140.0), &[])).unwrap();
    let with = analyse(&req(&x, Some(140.0), &moments)).unwrap();
    assert_eq!(bare.voice.f0_hz, with.voice.f0_hz);
    assert!(!bare.emotion.attached);
    assert!(with.emotion.attached);
    assert_ne!(profile_bytes(&bare), profile_bytes(&with));
}

#[test]
fn an_off_grid_moment_is_refused() {
    let x = synth::sine(SR as usize, SR, 140.0, 0.1);
    let bad = [MomentFrame {
        t_ms: 40,
        axes: vec![],
    }];
    let err = analyse(&req(&x, Some(140.0), &bad)).unwrap_err();
    assert!(err.contains("80"), "{err}");
}

#[test]
fn moment_parser_accepts_a_frame_list() {
    let text = r#"{"schema":"galion-preclone-moment-in/1","frames":[{"t_ms":0,"axes":[{"name":"valence","value":0.2}]}]}"#;
    let frames = parse_moments(text).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].t_ms, 0);
    assert_eq!(frames[0].axes[0].value, 0.2);
}

#[test]
fn the_moment_clock_still_matches_the_lock_and_the_feeling_core() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let locks = fs::read_to_string(root.join("LOCKS.toml")).unwrap();
    assert!(locks.contains("frame_ms = 80"));
    let feeling = fs::read_to_string(root.join("tools/symphony/feeling/src/frame.rs")).unwrap();
    assert!(feeling.contains("pub const TICK_MS: u32 = 80"));
    let beat = fs::read_to_string(root.join("crates/galion-heartbeat/src/lib.rs")).unwrap();
    assert!(beat.contains("pub const FRAME_MS: u32 = 80"));
    assert_eq!(galion_preclone::MOMENT_TICK_MS, 80);
}

#[test]
fn cli_reads_a_wav_and_writes_the_same_bytes_as_the_library() {
    let dir = std::env::temp_dir().join(format!("galion-preclone-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let wav_path = dir.join("ref.wav");
    let out_path = dir.join("profile.json");
    let x = synth::formant_voice(
        SR as usize,
        SR,
        140.0,
        0.05,
        &[(700.0, 90.0), (1500.0, 120.0), (2500.0, 150.0)],
    );
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
    for s in &x {
        writer.write_sample(*s).unwrap();
    }
    writer.finalize().unwrap();
    // Cargo's path includes the `.exe` suffix on Windows.
    let bin = env!("CARGO_BIN_EXE_galion-preclone");
    let status = Command::new(bin)
        .arg("--wav")
        .arg(&wav_path)
        .arg("--out")
        .arg(&out_path)
        .arg("--target-f0")
        .arg("140")
        .status()
        .unwrap();
    assert!(status.success());
    let (read, rate) = read_wav(&wav_path).unwrap();
    assert_eq!(rate, SR);
    assert_eq!(read.len(), x.len());
    let from_cli = fs::read(&out_path).unwrap();
    let from_lib = profile_bytes(&analyse(&req(&read, Some(140.0), &[])).unwrap());
    assert_eq!(from_cli, from_lib);
    let text = String::from_utf8(from_cli).unwrap();
    assert!(!text.contains("ref.wav"));
    let _ = fs::remove_dir_all(&dir);
}
