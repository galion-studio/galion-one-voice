// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Checker tests. Every signal is a sine or noise generated here. No voice audio.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use galion_refcheck::check::{check_pcm, Report, Role};
use galion_refcheck::dsp::{self, dominant_hz, estimate_rt60, spectral_tilt};
use galion_refcheck::manifest::{self, SessionFile};
use galion_refcheck::spec::{self, load_spec, Spec, EMOTIONS};
use galion_refcheck::synth::{self, burst, calibration_tone, shaped_noise};
use galion_refcheck::wav::{encode, parse_wav, EncodeSpec, SampleKind};

fn spec() -> Spec {
    load_spec(&repo().join("docs/emotion-v2/capture-spec.toml")).unwrap()
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn pcm24(samples: &[f32], sr: u32) -> galion_refcheck::Pcm {
    let bytes = encode(
        &EncodeSpec {
            sample_rate: sr,
            channels: 1,
            bits_per_sample: 24,
            kind: SampleKind::PcmInteger,
        },
        samples,
    )
    .unwrap();
    parse_wav(&bytes).unwrap()
}

fn emotion() -> (Spec, galion_refcheck::Pcm) {
    let spec = spec();
    let samples = burst(
        48_000,
        spec.spectral.tilt_db_per_octave,
        0.025,
        spec.loudness.target_lufs,
        11,
    )
    .unwrap();
    (spec, pcm24(&samples, 48_000))
}

fn assert_pass(report: &Report) {
    let failed: Vec<String> = report
        .checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| format!("{} value={:?} {}", c.name, c.value, c.detail))
        .collect();
    assert!(
        report.passed() && failed.is_empty(),
        "failed: {failed:?}\n{}",
        serde_json::to_string_pretty(report).unwrap()
    );
}

fn by_name<'a>(report: &'a Report, name: &str) -> &'a galion_refcheck::check::Check {
    report
        .checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("missing check {name}"))
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = chmod_tree(&self.0, 0o755, 0o644);
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn temp_dir(name: &str) -> TempDir {
    let p = std::env::temp_dir().join(format!("galion-refcheck-{name}-{}", std::process::id()));
    let _ = chmod_tree(&p, 0o755, 0o644);
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    TempDir(p)
}
fn chmod_tree(root: &Path, dir_mode: u32, file_mode: u32) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            chmod_tree(&path, dir_mode, file_mode)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(dir_mode))?;
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(file_mode))?;
        }
    }
    Ok(())
}

#[test]
fn spec_locks_the_432_standard() {
    let s = spec();
    assert_eq!(s.standard, "galion-voice-v2-432");
    assert_eq!(s.persona, "samantha");
    assert_eq!(s.consent, "owner-own-recordings-only");
    assert_eq!(s.reference_hz, 432.0);
    assert_eq!(s.format.sample_rate_hz, 48_000);
    assert_eq!(s.format.bit_depth, 24);
    assert_eq!(s.format.channels, 1);
    assert_eq!(s.format.sample_format, "pcm_integer");
    assert_eq!(s.loudness.target_lufs, -20.0);
    assert_eq!(s.loudness.tolerance_lu, 2.0);
    assert_eq!(s.true_peak.max_dbtp, -1.0);
    assert_eq!(s.true_peak.oversample, 4);
    assert_eq!(s.noise_floor.max_dbfs, -60.0);
    assert_eq!(s.noise_floor.window_ms, 400);
    assert_eq!(s.clipping.max_clipped_samples, 0);
    assert_eq!(s.dc_offset.max_dbfs, -50.0);
    assert_eq!(s.duration.min_s, 3.0);
    assert_eq!(s.duration.max_s, 12.0);
    assert_eq!(s.calibration.tone_hz, 432.0);
    assert_eq!(s.calibration.tolerance_hz, 2.0);
    assert_eq!(s.calibration.min_passing, 1);
    assert_eq!(s.reverb.max_rt60_s, 0.40);
    assert_eq!(s.spectral.tilt_db_per_octave, -6.0);
    assert_eq!(s.spectral.tolerance_db, 2.0);
    assert_eq!((s.spectral.octave_min, s.spectral.octave_max), (-2, 4));
    assert_eq!(s.takes.per_emotion, 5);
    assert_eq!(s.takes.min_per_emotion, 3);
    assert_eq!(s.emotions.names, EMOTIONS);
}

#[test]
fn protocol_quotes_the_gates() {
    let md = fs::read_to_string(repo().join("docs/emotion-v2/CAPTURE-PROTOCOL.md")).unwrap();
    for needle in [
        "48000",
        "-20.0 LUFS",
        "-1.0 dBTP",
        "-60 dBFS",
        "-50 dBFS",
        "0.40 s",
        "-6.0 dB/octave",
        "432 Hz",
        "30 cm",
        "owner-own-recordings-only",
        "Neutral",
        "Disgusted",
        "sha256",
    ] {
        assert!(md.contains(needle), "protocol is missing {needle}");
    }
}

#[test]
fn no_audio_fixtures_in_the_emotion_v2_tree() {
    for rel in ["docs/emotion-v2", "tools/emotion-v2"] {
        let root = repo().join(rel);
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                for ext in [".wav", ".wave", ".mp3", ".flac", ".ogg"] {
                    assert!(
                        !name.ends_with(ext),
                        "audio committed at {}",
                        path.display()
                    );
                }
            }
        }
    }
}

#[test]
fn emotion_take_of_shaped_noise_passes() {
    let (spec, pcm) = emotion();
    let report = check_pcm(&pcm, &spec, Role::Emotion);
    assert_pass(&report);
    let tp = by_name(&report, "true_peak").value.unwrap();
    assert!(tp < -3.0, "need headroom under the true-peak cap, got {tp}");
    assert!(report.checks.iter().all(|c| c.name != "calibration_tone"));
    assert!(report.checks.iter().any(|c| c.name == "spectral_tilt"));
}

#[test]
fn calibration_sine_passes() {
    let spec = spec();
    let samples = calibration_tone(48_000, spec.loudness.target_lufs).unwrap();
    let report = check_pcm(&pcm24(&samples, 48_000), &spec, Role::Calibration);
    assert_pass(&report);
    let hz = by_name(&report, "calibration_tone").value.unwrap();
    assert!((hz - 432.0).abs() < 1.0, "dominant {hz}");
    assert!(report.checks.iter().all(|c| c.name != "spectral_tilt"));
}

#[test]
fn sample_rate_channel_and_bit_depth_fail_on_their_own_rows() {
    let (spec, pcm) = emotion();
    let mut wrong_rate = pcm.clone();
    wrong_rate.sample_rate = 44_100;
    let report = check_pcm(&wrong_rate, &spec, Role::Emotion);
    assert!(!report.passed());
    assert!(!by_name(&report, "sample_rate").pass);

    let bytes = encode(
        &EncodeSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 16,
            kind: SampleKind::PcmInteger,
        },
        &synth::sine(48_000 * 4, 48_000, 432.0, 0.1),
    )
    .unwrap();
    let report = check_pcm(&parse_wav(&bytes).unwrap(), &spec, Role::Emotion);
    assert!(!by_name(&report, "bit_depth").pass);
    assert_eq!(by_name(&report, "bit_depth").value, Some(16.0));

    let mono = emotion().1.samples.clone();
    let mut stereo = Vec::with_capacity(mono.len() * 2);
    for s in &mono {
        stereo.push(*s);
        stereo.push(*s);
    }
    let bytes = encode(
        &EncodeSpec {
            sample_rate: 48_000,
            channels: 2,
            bits_per_sample: 24,
            kind: SampleKind::PcmInteger,
        },
        &stereo,
    )
    .unwrap();
    let report = check_pcm(&parse_wav(&bytes).unwrap(), &spec, Role::Emotion);
    assert!(!by_name(&report, "channels").pass);
    assert_eq!(by_name(&report, "channels").value, Some(2.0));
}

#[test]
fn loudness_window_rejects_a_shift_of_four_decibels() {
    let (spec, pcm) = emotion();
    for (db, hotter) in [(4.0, true), (-4.0, false)] {
        let g = 10f32.powf(db / 20.0);
        let scaled: Vec<f32> = pcm.samples.iter().map(|s| s * g).collect();
        let report = check_pcm(&pcm24(&scaled, 48_000), &spec, Role::Emotion);
        let row = by_name(&report, "integrated_lufs");
        assert!(
            !row.pass,
            "{db} dB should miss the window, value {:?}",
            row.value
        );
        let v = row.value.unwrap();
        if hotter {
            assert!(v > spec.loudness.target_lufs + spec.loudness.tolerance_lu);
        } else {
            assert!(v < spec.loudness.target_lufs - spec.loudness.tolerance_lu);
        }
    }
}

#[test]
fn true_peak_clipping_dc_and_duration_each_fail() {
    let spec = spec();
    let hot = synth::sine(48_000 * 3, 48_000, 1000.0, 1.0);
    let report = check_pcm(&pcm24(&hot, 48_000), &spec, Role::Emotion);
    assert!(!by_name(&report, "true_peak").pass);
    assert!(by_name(&report, "true_peak").value.unwrap() > -1.0);
    assert!(by_name(&report, "clipping").value.unwrap() >= 1.0);

    let (spec, pcm) = emotion();
    let mut clipped = pcm.samples.clone();
    let mid = clipped.len() / 2;
    clipped[mid] = 1.0;
    let report = check_pcm(&pcm24(&clipped, 48_000), &spec, Role::Emotion);
    assert!(!by_name(&report, "clipping").pass);
    assert!(by_name(&report, "clipping").value.unwrap() >= 1.0);

    let mut biased: Vec<f32> = pcm.samples.iter().map(|s| s + 0.02).collect();
    // Keep the rail clear: the added offset must not quantize to full scale.
    for s in &mut biased {
        *s = s.clamp(-0.9, 0.9);
    }
    let report = check_pcm(&pcm24(&biased, 48_000), &spec, Role::Emotion);
    let dc = by_name(&report, "dc_offset");
    assert!(!dc.pass, "dc {:?}", dc.value);
    assert!(dc.value.unwrap() > spec.dc_offset.max_dbfs);

    let short = pcm24(&pcm.samples[..48_000], 48_000);
    let report = check_pcm(&short, &spec, Role::Emotion);
    assert!(!by_name(&report, "duration").pass);
    assert!(by_name(&report, "duration").value.unwrap() < spec.duration.min_s);
}

#[test]
fn noise_floor_uses_the_quietest_window() {
    let spec = spec();
    let mut x = vec![0.0f32; 48_000 * 2];
    // 0.6 s at about -70 dBFS, then the rest at about -30 dBFS.
    let quiet = 10f32.powf(-70.0 / 20.0);
    let loud = 10f32.powf(-30.0 / 20.0);
    for (i, s) in x.iter_mut().enumerate() {
        let amp = if i < 48_000 * 6 / 10 { quiet } else { loud };
        *s = amp * ((i * 17) % 1000) as f32 / 500.0 - amp;
    }
    let floor = dsp::noise_floor_dbfs(&x, 48_000, &spec.noise_floor).unwrap();
    assert!(floor < -60.0 && floor > -95.0, "quietest window {floor}");
    for s in &mut x[..48_000 * 6 / 10] {
        *s *= 10f32.powf(30.0 / 20.0); // raise the bed to about -40 dBFS
    }
    let floor = dsp::noise_floor_dbfs(&x, 48_000, &spec.noise_floor).unwrap();
    assert!(floor > spec.noise_floor.max_dbfs, "raised bed {floor}");
}

#[test]
fn shaped_tilt_and_white_tilt_separate() {
    let spec = spec();
    let shaped = shaped_noise(48_000 * 2, 48_000, -6.0, 3);
    let (slope, _) = spectral_tilt(&shaped, 48_000, 432.0, &spec.spectral).unwrap();
    assert!((slope + 6.0).abs() < 1.0, "shaped tilt {slope}");

    let white = shaped_noise(48_000 * 2, 48_000, 0.0, 5);
    let (slope, _) = spectral_tilt(&white, 48_000, 432.0, &spec.spectral).unwrap();
    assert!(slope.abs() < 1.0, "white tilt {slope}");

    let samples = burst(48_000, 0.0, 0.025, spec.loudness.target_lufs, 21).unwrap();
    let report = check_pcm(&pcm24(&samples, 48_000), &spec, Role::Emotion);
    assert!(
        !by_name(&report, "spectral_tilt").pass,
        "{:?}",
        by_name(&report, "spectral_tilt")
    );
    assert!(!report.passed());
}

#[test]
fn reverb_estimate_passes_a_short_tail_and_fails_a_long_one() {
    let spec = spec();
    let short = burst(48_000, -6.0, 0.025, -20.0, 31).unwrap();
    let rt = estimate_rt60(&short, 48_000, &spec.reverb).unwrap();
    assert!(rt < spec.reverb.max_rt60_s, "short tail {rt}");
    assert!(
        rt > 0.02,
        "short tail should still be a measured decay, got {rt}"
    );

    let long = burst(48_000, -6.0, 0.20, -20.0, 33).unwrap();
    let report = check_pcm(&pcm24(&long, 48_000), &spec, Role::Emotion);
    let row = by_name(&report, "reverb");
    assert!(!row.pass, "long tail {:?}", row.value);
    assert!(row.value.unwrap_or(0.0) > spec.reverb.max_rt60_s);
}

#[test]
fn dominant_frequency_of_a_432_hz_sine() {
    let x = synth::sine(48_000 * 2, 48_000, 432.0, 0.2);
    let hz = dominant_hz(&x, 48_000).unwrap();
    assert!((hz - 432.0).abs() < 0.5, "{hz}");
}

#[test]
fn a_widened_consent_fails_the_identity_check() {
    let (mut spec, pcm) = emotion();
    spec.consent = "any-recording".into();
    let report = check_pcm(&pcm, &spec, Role::Emotion);
    assert!(!by_name(&report, "spec_identity").pass);
    assert!(!report.passed());
}

#[test]
fn session_seal_counts_passing_takes_for_all_twelve() {
    let spec = spec();
    let mut files = Vec::new();
    for name in EMOTIONS {
        let slug = spec::slug(name);
        for n in 1..=3 {
            files.push(SessionFile {
                path: format!("{slug}/take-0{n}.wav"),
                sha256: "ab".into(),
                bytes: 1,
                emotion: Some(slug.clone()),
                role: "emotion".into(),
                verdict: Some("PASS".into()),
            });
        }
    }
    files.push(SessionFile {
        path: "calibration/take-01.wav".into(),
        sha256: "cd".into(),
        bytes: 1,
        emotion: None,
        role: "calibration".into(),
        verdict: Some("PASS".into()),
    });
    assert!(
        manifest::session_gaps(&files, &spec).is_empty(),
        "{:?}",
        manifest::session_gaps(&files, &spec)
    );
    files.retain(|f| f.path != "angry/take-03.wav");
    let gaps = manifest::session_gaps(&files, &spec);
    assert!(gaps.iter().any(|g| g.starts_with("angry")), "{gaps:?}");
    files.retain(|f| f.role != "calibration");
    let gaps = manifest::session_gaps(&files, &spec);
    assert!(
        gaps.iter().any(|g| g.starts_with("calibration")),
        "{gaps:?}"
    );
}

#[test]
fn manifest_hashes_are_stable_and_a_changed_byte_fails() {
    let dir = temp_dir("hash");
    let wav = dir.0.join("neutral").join("take-01.wav");
    fs::create_dir_all(wav.parent().unwrap()).unwrap();
    let bytes = encode(
        &EncodeSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 24,
            kind: SampleKind::PcmInteger,
        },
        &synth::sine(4_800, 48_000, 432.0, 0.1),
    )
    .unwrap();
    fs::write(&wav, &bytes).unwrap();
    let manifest = manifest::build_manifest(&dir.0, None).unwrap();
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].emotion.as_deref(), Some("neutral"));
    assert_eq!(manifest.files[0].bytes, bytes.len() as u64);
    assert!(manifest.files[0].verdict.is_none());
    let path = dir.0.join("MANIFEST.json");
    fs::write(&path, manifest::to_json(&manifest).unwrap()).unwrap();
    fs::set_permissions(&wav, fs::Permissions::from_mode(0o444)).unwrap();
    let spec = spec();
    let report = manifest::verify_manifest(&path, &spec, false, false).unwrap();
    assert_eq!(report.verdict, "PASS", "{report:?}");

    fs::set_permissions(&wav, fs::Permissions::from_mode(0o644)).unwrap();
    let mut flipped = bytes.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 0x01;
    fs::write(&wav, &flipped).unwrap();
    fs::set_permissions(&wav, fs::Permissions::from_mode(0o444)).unwrap();
    let report = manifest::verify_manifest(&path, &spec, false, false).unwrap();
    assert_eq!(report.verdict, "FAIL");
    assert!(
        report.mismatches.iter().any(|m| m.contains("sha256")),
        "{:?}",
        report.mismatches
    );
}

#[test]
fn a_rewritten_verdict_does_not_seal() {
    let dir = temp_dir("verdict");
    let wav = dir.0.join("neutral").join("take-01.wav");
    fs::create_dir_all(wav.parent().unwrap()).unwrap();
    let bytes = encode(
        &EncodeSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 24,
            kind: SampleKind::PcmInteger,
        },
        &synth::sine(4_800, 48_000, 432.0, 0.1),
    )
    .unwrap();
    fs::write(&wav, &bytes).unwrap();
    let spec = spec();
    let mut manifest = manifest::build_manifest(&dir.0, Some(&spec)).unwrap();
    assert_eq!(manifest.files[0].verdict.as_deref(), Some("FAIL"));
    manifest.files[0].verdict = Some("PASS".into());
    let path = dir.0.join("MANIFEST.json");
    fs::write(&path, manifest::to_json(&manifest).unwrap()).unwrap();
    fs::set_permissions(&wav, fs::Permissions::from_mode(0o444)).unwrap();
    let report = manifest::verify_manifest(&path, &spec, true, false).unwrap();
    assert_eq!(report.verdict, "FAIL");
    assert!(
        report.mismatches.iter().any(|m| m.contains("verdict")),
        "{:?}",
        report.mismatches
    );
}

#[test]
fn cli_json_reports_fail_and_pass() {
    let dir = temp_dir("cli");
    let spec_path = repo().join("docs/emotion-v2/capture-spec.toml");
    let bad = dir.0.join("bad.wav");
    let bytes = encode(
        &EncodeSpec {
            sample_rate: 22_050,
            channels: 1,
            bits_per_sample: 16,
            kind: SampleKind::PcmInteger,
        },
        &synth::sine(22_050, 22_050, 440.0, 0.2),
    )
    .unwrap();
    fs::write(&bad, bytes).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_refcheck"))
        .args([
            "check",
            "--wav",
            bad.to_str().unwrap(),
            "--spec",
            spec_path.to_str().unwrap(),
            "--role",
            "emotion",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["verdict"], "FAIL");
    assert_eq!(v["tool"], "refcheck");
    assert!(v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "sample_rate" && c["pass"] == false));

    let spec = spec();
    let good_samples = burst(
        48_000,
        spec.spectral.tilt_db_per_octave,
        0.025,
        spec.loudness.target_lufs,
        11,
    )
    .unwrap();
    let good = dir.0.join("good.wav");
    fs::write(
        &good,
        encode(
            &EncodeSpec {
                sample_rate: 48_000,
                channels: 1,
                bits_per_sample: 24,
                kind: SampleKind::PcmInteger,
            },
            &good_samples,
        )
        .unwrap(),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_refcheck"))
        .args([
            "check",
            "--wav",
            good.to_str().unwrap(),
            "--spec",
            spec_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(v["verdict"], "PASS");
    assert_eq!(v["role"], "emotion");
}
