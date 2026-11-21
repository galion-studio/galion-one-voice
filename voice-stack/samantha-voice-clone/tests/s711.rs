// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use samantha_voice_clone::{
    probe_compute, run_repo_stub_probe, run_self_check, s711_worker_script, sentences, Pad,
    Reference, SamanthaTts, Tts, WorkerSpeaker, FRAME_SAMPLES, REF_SHA_PREFIX,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("galion-s711-{name}-{}-{nanos}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn sector_gitignore_names_the_voice_suffixes() {
    let text = include_str!("../.gitignore");
    for ext in [".wav", ".mp3", ".flac", ".onnx"] {
        assert!(text.contains(ext), "{ext} missing from sector gitignore");
    }
}

#[test]
fn no_voice_file_is_tracked_under_voice_stack() {
    let output = Command::new("git")
        .args(["ls-files", "--", "voice-stack"])
        .current_dir(env_repo())
        .output()
        .expect("git ls-files");
    assert!(output.status.success(), "git ls-files failed");
    let listed = String::from_utf8_lossy(&output.stdout);
    for line in listed.lines() {
        let lower = line.to_ascii_lowercase();
        for ext in [".wav", ".mp3", ".flac", ".onnx"] {
            assert!(!lower.ends_with(ext), "tracked voice file: {line}");
        }
    }
}

#[test]
fn worker_source_does_not_download_and_names_the_recipe() {
    let source = fs::read_to_string(s711_worker_script()).unwrap();
    assert!(source.contains("SPDX-License-Identifier: MIT"));
    assert!(source.contains("Copyright (c) 2026 Galion Labs"));
    assert!(source.contains("10ac44dc"));
    assert!(source.contains("local-zero-shot-conditioning"));
    assert!(source.contains("chatterbox_worker"));
    assert!(!source.contains("from_pretrained"));
    assert!(!source.contains("huggingface.co"));
    assert_eq!(sentences("Hello. Again!").len(), 2);
}

#[test]
fn self_check_and_repo_stub_match_the_s711_knobs() {
    let check = run_self_check().expect("self-check");
    assert_eq!(check.reuse, "chatterbox_worker");
    assert_eq!(check.mode, "expr");
    assert_eq!(check.exaggeration, "0.5");
    assert_eq!(check.cfg, "0.5");
    assert_eq!(check.seed0, 711);
    assert_eq!(check.seed1, 712);
    assert!(check.prefix_ok);
    assert!(!check.prefix_bad);
    assert_eq!(check.chain, "refused");
    assert_eq!(check.train, "refused");
    assert_eq!(check.wrong_seed, "refused");
    assert_eq!(check.stub_samples, FRAME_SAMPLES);
    assert_eq!(check.resample_out, 2);
    assert_eq!(check.consent, "local-zero-shot-conditioning");
    assert!(check.wall_us > 0, "self-check wall time was 0");
    assert!(check.device_probe == "cpu" || check.device_probe == "cuda");
    assert_eq!(check.auto_device, "cpu");

    let repo = run_repo_stub_probe().expect("repo stub");
    assert_eq!(repo.exit_code, 0);
    assert_eq!(repo.pcm_bytes, FRAME_SAMPLES * 2);
    // stub_tone("Hello.") with exaggeration 0.5 and cfg 0.5:
    // 1000 + len%50 + round(0.5*1000) + round(0.5*500) = 1006 + 500 + 250.
    assert_eq!(repo.first_sample, 1756, "controls={}", repo.controls_line);
    assert!(
        repo.controls_line.contains("exaggeration=0.5000"),
        "{}",
        repo.controls_line
    );
    assert!(
        repo.controls_line.contains("cfg_weight=0.5000"),
        "{}",
        repo.controls_line
    );
    assert!(
        repo.controls_line.contains("rate=1.0000"),
        "{}",
        repo.controls_line
    );
    assert!(repo.wall_us > 0, "repo stub wall time was 0");
    assert!(!repo.pcm_bytes.to_string().is_empty());
}

#[test]
fn a_wrong_reference_is_refused_by_the_worker_and_by_from_file() {
    let dir = scratch("ref");
    let path = dir.join("fixture.wav");
    fs::write(&path, b"prefix gate fixture, not a voice").unwrap();
    let err = Reference::from_file(&path).unwrap_err();
    assert_eq!(err.kind, "reference");
    assert!(err.detail.contains(REF_SHA_PREFIX));

    let trusted =
        Reference::trusted_for_tests(path.clone(), format!("{REF_SHA_PREFIX}{}", "ab".repeat(28)))
            .unwrap();
    let mut voice = SamanthaTts::with_speaker(
        trusted,
        WorkerSpeaker::spawn("cpu", true).unwrap(),
        None,
        "cpu",
    );
    let err = voice.synthesize("Hello.", &Pad::RESTING).unwrap_err();
    assert_eq!(err.kind, "worker");
    assert!(
        err.detail.contains("10ac44dc") || err.detail.contains("prefix"),
        "{err}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn compute_probe_returns() {
    let probe = probe_compute();
    assert!(probe.elapsed_us < 30_000_000, "{probe:?}");
    assert!(!probe.note.is_empty());
    match probe.raw.as_str() {
        "cuda" => assert_eq!(probe.kind.as_str(), "cuda"),
        "cpu" | "" => assert_eq!(probe.kind.as_str(), "cpu"),
        other => panic!("probe stdout {other}"),
    }
}

fn env_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
