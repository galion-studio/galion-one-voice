// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic tones only. No person audio, no download, no timed measurement.

use galion_voice_guard::{admit, before_listen, check, check_frames, Config, Context, Pcm, SCHEMA};

const SR: u32 = 16_000;

fn cfg() -> Config {
    Config::bundled().expect("bundled config")
}

fn sine(hz: f64, secs: f64, amp: f32) -> Vec<f32> {
    let n = (secs * SR as f64).round() as usize;
    (0..n)
        .map(|i| {
            let phase = 2.0 * std::f64::consts::PI * hz * i as f64 / SR as f64;
            (amp as f64 * phase.sin()) as f32
        })
        .collect()
}

fn noise(n: usize, amp: f32, seed: u32) -> Vec<f32> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let u = ((x >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0;
            (u * 2.0 - 1.0) * amp
        })
        .collect()
}

fn judge(samples: &[f32], sr: u32) -> galion_voice_guard::Report {
    check(
        Pcm {
            interleaved: samples,
            sample_rate_hz: sr,
            channels: 1,
            bits_per_sample: None,
        },
        &cfg(),
        &Context::clip(),
    )
    .expect("check")
}

fn row<'a>(report: &'a galion_voice_guard::Report, name: &str) -> &'a galion_voice_guard::Check {
    report
        .checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn clean_tone_passes_and_the_report_is_stable() {
    let y = sine(180.0, 0.8, 0.2);
    let a = judge(&y, SR);
    let b = judge(&y, SR);
    assert_eq!(a.schema, SCHEMA);
    assert_eq!(a.verdict, "pass", "{}", a.reason);
    assert!(!a.bad);
    assert!(a.bad_frames.is_empty());
    assert_eq!(row(&a, "neural_quality").status, "stub");
    assert_eq!(row(&a, "speaker_drift").status, "stub");
    for name in [
        "finite",
        "denormal",
        "dc",
        "clipping",
        "true_peak",
        "click",
        "ar_click",
        "dropout",
        "repeated_frame",
        "babble",
        "buzz",
        "cutoff",
        "format",
    ] {
        assert_eq!(
            row(&a, name).status,
            "pass",
            "{name}: {}",
            row(&a, name).reason
        );
    }
    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb);
}

#[test]
fn nan_is_caught_and_admit_refuses_it() {
    let mut y = sine(180.0, 0.8, 0.2);
    y[100] = f32::NAN;
    let report = judge(&y, SR);
    assert_eq!(report.verdict, "fail");
    assert_eq!(row(&report, "finite").status, "fail");
    assert!(!row(&report, "finite").frames.is_empty());
    let refused = admit(
        Pcm {
            interleaved: &y,
            sample_rate_hz: SR,
            channels: 1,
            bits_per_sample: None,
        },
        &cfg(),
        &Context::clip(),
    );
    assert!(refused.is_err());
}

#[test]
fn denormal_is_caught() {
    let mut y = sine(180.0, 0.8, 0.2);
    y[200] = f32::from_bits(1);
    assert!(y[200].is_subnormal());
    let report = judge(&y, SR);
    assert_eq!(row(&report, "denormal").status, "fail");
    assert_eq!(report.verdict, "fail");
}

#[test]
fn dc_offset_is_caught() {
    let y: Vec<f32> = sine(180.0, 0.8, 0.2)
        .into_iter()
        .map(|s| s + 0.05)
        .collect();
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "dc").status,
        "fail",
        "{}",
        row(&report, "dc").reason
    );
    assert!(row(&report, "dc").value.unwrap() > -50.0);
}

#[test]
fn clipping_and_true_peak_are_caught() {
    let y: Vec<f32> = sine(180.0, 0.8, 0.2)
        .into_iter()
        .map(|s| (s * 8.0).clamp(-1.0, 1.0))
        .collect();
    let report = judge(&y, SR);
    assert_eq!(row(&report, "clipping").status, "fail");
    assert_eq!(row(&report, "true_peak").status, "fail");
    assert!(!row(&report, "clipping").frames.is_empty());
}

#[test]
fn click_is_caught() {
    let mut y = sine(180.0, 0.8, 0.2);
    let i = y.len() / 2;
    y[i] = 1.0;
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "click").status,
        "fail",
        "{}",
        row(&report, "click").reason
    );
    assert_eq!(
        row(&report, "ar_click").status,
        "fail",
        "{}",
        row(&report, "ar_click").reason
    );
    assert!(row(&report, "click")
        .frames
        .iter()
        .any(|s| i as u32 / 1280 >= s.start && i as u32 / 1280 < s.end));
}

#[test]
fn dropout_is_caught() {
    let mut y = sine(180.0, 0.8, 0.2);
    let a = SR as usize * 2 / 10;
    let b = a + SR as usize * 25 / 100;
    for s in &mut y[a..b] {
        *s = 0.0;
    }
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "dropout").status,
        "fail",
        "{}",
        row(&report, "dropout").reason
    );
    assert!(!row(&report, "dropout").frames.is_empty());
}

#[test]
fn repeated_frames_are_caught() {
    let mut y = sine(180.0, 0.8, 0.2);
    let fl = 1280;
    let src = 2 * fl;
    let frame = y[src..src + fl].to_vec();
    for k in 3..6 {
        y[k * fl..(k + 1) * fl].copy_from_slice(&frame);
    }
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "repeated_frame").status,
        "fail",
        "{}",
        row(&report, "repeated_frame").reason
    );
    assert!(!row(&report, "repeated_frame").frames.is_empty());
}

#[test]
fn hard_mute_ending_is_a_cutoff() {
    let mut y = sine(180.0, 0.8, 0.2);
    let tail = SR as usize * 40 / 1000;
    let cut = y.len() - tail;
    y[cut - 1] = 0.8;
    for s in &mut y[cut..] {
        *s = 0.0;
    }
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "cutoff").status,
        "fail",
        "{}",
        row(&report, "cutoff").reason
    );
}

#[test]
fn white_noise_is_babble_and_a_tone_is_not() {
    let n = (0.8 * SR as f64) as usize;
    let report = judge(&noise(n, 0.08, 13), SR);
    assert_eq!(
        row(&report, "babble").status,
        "fail",
        "value {:?}",
        row(&report, "babble").value
    );
    let tone = judge(&sine(180.0, 0.8, 0.2), SR);
    assert_eq!(
        row(&tone, "babble").status,
        "pass",
        "{}",
        row(&tone, "babble").reason
    );
}

#[test]
fn noise_dominated_tone_is_buzz() {
    let tone = sine(180.0, 0.8, 0.2);
    let n = noise(tone.len(), 0.30, 7);
    let y: Vec<f32> = tone.iter().zip(n).map(|(a, b)| a + b).collect();
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "buzz").status,
        "fail",
        "{}",
        row(&report, "buzz").reason
    );
    let clean = judge(&sine(180.0, 0.8, 0.2), SR);
    assert_eq!(
        row(&clean, "buzz").status,
        "pass",
        "{}",
        row(&clean, "buzz").reason
    );
}

#[test]
fn wrong_sample_rate_fails_format() {
    let y = sine(180.0, 0.8, 0.2);
    let report = judge(&y, 8_000);
    assert_eq!(
        row(&report, "format").status,
        "fail",
        "{}",
        row(&report, "format").reason
    );
    assert_eq!(report.verdict, "fail");
}

#[test]
fn short_frame_fails_closed() {
    let y = sine(180.0, 0.08, 0.2);
    let short = &y[..100];
    let report = check_frames(&[short], SR, &cfg(), &Context::clip()).expect("frames");
    assert_eq!(report.verdict, "fail");
    assert_eq!(row(&report, "format").status, "fail");
}

#[test]
fn same_tone_keeps_the_speaker_and_an_octave_does_not() {
    let a = sine(180.0, 1.0, 0.2);
    let cfg = cfg();
    let same = check(
        Pcm {
            interleaved: &a,
            sample_rate_hz: SR,
            channels: 1,
            bits_per_sample: None,
        },
        &cfg,
        &Context {
            baseline: Some(galion_voice_guard::Baseline {
                samples: &a,
                sample_rate_hz: SR,
            }),
            embedding_cos: None,
            expect_release: true,
            wav_path: None,
        },
    )
    .unwrap();
    assert_eq!(
        row(&same, "speaker_drift").status,
        "pass",
        "{}",
        row(&same, "speaker_drift").reason
    );
    assert!(
        row(&same, "speaker_drift").score.unwrap() >= 0.85,
        "{}",
        row(&same, "speaker_drift").reason
    );

    let low = sine(90.0, 1.0, 0.2);
    let other = check(
        Pcm {
            interleaved: &low,
            sample_rate_hz: SR,
            channels: 1,
            bits_per_sample: None,
        },
        &cfg,
        &Context {
            baseline: Some(galion_voice_guard::Baseline {
                samples: &a,
                sample_rate_hz: SR,
            }),
            embedding_cos: None,
            expect_release: true,
            wav_path: None,
        },
    )
    .unwrap();
    assert_eq!(
        row(&other, "speaker_drift").status,
        "fail",
        "{}",
        row(&other, "speaker_drift").reason
    );
    assert!(row(&other, "speaker_drift").score.unwrap() < 0.55);
}

#[test]
fn caller_ecapa_cosine_below_the_merge_gate_fails() {
    let y = sine(180.0, 0.8, 0.2);
    let report = check(
        Pcm {
            interleaved: &y,
            sample_rate_hz: SR,
            channels: 1,
            bits_per_sample: None,
        },
        &cfg(),
        &Context {
            baseline: None,
            embedding_cos: Some(0.2),
            expect_release: true,
            wav_path: None,
        },
    )
    .unwrap();
    assert_eq!(row(&report, "speaker_drift").status, "fail");
    assert_eq!(report.verdict, "fail");
}

#[test]
fn filtered_impulse_train_is_not_a_sample_jump_click() {
    let y = impulse_vowel(180.0, 0.8);
    let report = judge(&y, SR);
    assert_eq!(
        row(&report, "click").status,
        "pass",
        "jump {}",
        row(&report, "click").value.unwrap()
    );
    assert_eq!(
        row(&report, "ar_click").status,
        "pass",
        "{}",
        row(&report, "ar_click").reason
    );
}

#[test]
fn before_listen_admits_a_clean_tone() {
    let y = sine(180.0, 0.8, 0.2);
    let ok = before_listen(&y, SR, &cfg(), None, None);
    assert!(ok.is_ok(), "{}", ok.unwrap_err().reason);
}

fn impulse_vowel(f0: f64, secs: f64) -> Vec<f32> {
    let sr = SR as f64;
    let n = (secs * sr).round() as usize;
    let mut y = vec![0.0f64; n];
    let period = (sr / f0).round().max(1.0) as usize;
    let mut i = 0;
    while i < n {
        y[i] = 1.0;
        i += period;
    }
    for (f, bw) in [(700.0, 80.0), (1200.0, 120.0)] {
        let r = (-std::f64::consts::PI * bw / sr).exp();
        let c = 2.0 * r * (2.0 * std::f64::consts::PI * f / sr).cos();
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in &mut y {
            let o = *v + c * y1 - r * r * y2;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let m = y.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-9);
    y.into_iter().map(|v| (0.5 * v / m) as f32).collect()
}
