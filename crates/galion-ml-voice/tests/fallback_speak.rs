// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Fallback speak with no weight download and no network.
//!
//! `SpeechStack::detect` tries the clone slot, then the A21 Piper pack, then
//! libflite. This test points discovery at a refused marker outside the repo
//! and an empty clone directory, then speaks. It does not run a fetch script.
#![cfg(feature = "speech")]

use galion_ml_voice::speech::{weights::WeightStatus, SpeechBackend, SpeechStack, SPEECH_RATE};
use galion_ml_voice::VoiceError;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const PHRASE: &str = "hello galion";

#[test]
fn fallback_speak_with_no_weight_download_and_no_network() {
    let scratch = Scratch::new("galion-fallback-speak");
    let _env = EnvGuard::capture(&[
        "GALION_TTS_WEIGHTS",
        "GALION_SPEECH_WEIGHTS",
        "GALION_TTS_WEIGHTS_SHA256",
        "GALION_SPEECH",
        "GALION_CLONE_FETCH",
        "GALION_CLONE_WEIGHTS",
        "GALION_CLONE_CKPT",
        "GALION_CLONE_TOKENIZER",
        "GALION_CLONE_LOCAL_REFS",
        "GALION_CLONE_STUB_PCM",
        "GALION_CLONE_MODEL",
        "GALION_CLONE_WORKER",
        "GALION_A21_RUNTIME",
        "GALION_PIPER_LEGACY",
        "GALION_FLITE_VOICE",
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_DATASETS_OFFLINE",
    ]);

    let marker = scratch.path.join("refused.pt");
    fs::write(&marker, b"not-a-weight").unwrap();
    let repo_weights = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../weights");
    let marker_abs = marker.canonicalize().unwrap();
    if let Ok(weights_abs) = repo_weights.canonicalize() {
        assert!(
            !marker_abs.starts_with(&weights_abs),
            "refused marker must stay outside {}",
            weights_abs.display()
        );
    }

    std::env::set_var("GALION_TTS_WEIGHTS", &marker);
    std::env::remove_var("GALION_SPEECH_WEIGHTS");
    std::env::remove_var("GALION_TTS_WEIGHTS_SHA256");
    std::env::remove_var("GALION_CLONE_FETCH");
    std::env::set_var("GALION_CLONE_WEIGHTS", &scratch.path);
    std::env::remove_var("GALION_CLONE_CKPT");
    std::env::remove_var("GALION_CLONE_TOKENIZER");
    std::env::remove_var("GALION_CLONE_LOCAL_REFS");
    std::env::remove_var("GALION_CLONE_STUB_PCM");
    std::env::remove_var("GALION_CLONE_MODEL");
    std::env::remove_var("GALION_CLONE_WORKER");
    std::env::remove_var("GALION_A21_RUNTIME");
    std::env::remove_var("GALION_PIPER_LEGACY");
    std::env::set_var("GALION_FLITE_VOICE", "slt");
    std::env::set_var("HF_HUB_OFFLINE", "1");
    std::env::set_var("TRANSFORMERS_OFFLINE", "1");
    std::env::set_var("HF_DATASETS_OFFLINE", "1");

    let weights_before = snapshot_weights();
    let sockets_before = socket_fds();

    let status = WeightStatus::discover();
    assert_eq!(status.id(), "refused", "{}", status.summary());
    assert!(status.summary().contains("REFUSED"), "{}", status.summary());
    assert!(
        status.summary().contains("refused.pt"),
        "{}",
        status.summary()
    );
    eprintln!("weight_status={}", status.summary());

    for mode in ["auto", "clone", "flite"] {
        std::env::set_var("GALION_SPEECH", mode);
        let mut stack = SpeechStack::detect();
        speak_fallback(&mut stack, mode);
        stack.prewarm();
        let warmth = stack.warmth();
        assert_eq!(
            warmth.piper_pid, None,
            "{mode}: fallback must not start Piper"
        );
        assert_eq!(warmth.piper_spawns, 0, "{mode}");
        assert_eq!(
            warmth.clone_pid, None,
            "{mode}: fallback must not start the clone worker"
        );
        assert_eq!(warmth.clone_spawns, 0, "{mode}");
        assert!(stack.clone_frames().is_none(), "{mode}");
    }

    std::env::set_var("GALION_SPEECH", "a21");
    let mut a21 = SpeechStack::detect();
    assert!(!a21.can_speak(), "{}", a21.backend.label());
    match &a21.backend {
        SpeechBackend::None { reason } => {
            assert!(reason.contains("GALION_SPEECH=a21"), "{reason}");
            assert!(reason.contains("REFUSED"), "{reason}");
            assert!(!reason.contains("libflite"), "{reason}");
            eprintln!("a21_without_pack={reason}");
        }
        other => panic!("a21 without a pack must stay silent, got {}", other.label()),
    }
    let err = a21.speak(PHRASE).expect_err("a21 must not invent speech");
    assert!(matches!(err, VoiceError::Backend(_)), "{err}");
    assert!(err.to_string().contains("GALION_SPEECH=a21"), "{err}");
    a21.prewarm();
    assert_eq!(a21.warmth().piper_spawns, 0);

    assert!(std::env::var_os("GALION_CLONE_FETCH").is_none());
    assert_eq!(
        socket_fds(),
        sockets_before,
        "fallback speak left a socket open"
    );
    assert_eq!(
        snapshot_weights(),
        weights_before,
        "fallback speak wrote or downloaded a weight"
    );
    let scratch_names = dir_names(&scratch.path);
    assert_eq!(
        scratch_names,
        vec!["refused.pt".to_string()],
        "{scratch_names:?}"
    );
}

fn speak_fallback(stack: &mut SpeechStack, mode: &str) {
    let token = stack.backend.speech_token();
    let label = stack.backend.label();
    match token {
        "flite" => {
            let voice = match &stack.backend {
                SpeechBackend::SystemFlite { voice } => voice.clone(),
                other => panic!("{mode}: token flite but {}", other.label()),
            };
            assert_eq!(voice, "slt", "{mode}");
            assert!(label.contains("fallback system TTS"), "{label}");
            assert!(label.contains("NOT A21"), "{label}");
            assert!(label.contains("NOT neural"), "{label}");
            assert!(stack.can_speak(), "{mode}");
            let first = stack
                .speak(PHRASE)
                .unwrap_or_else(|e| panic!("{mode}: {e}"));
            let second = stack
                .speak(PHRASE)
                .unwrap_or_else(|e| panic!("{mode} repeat: {e}"));
            assert_eq!(first.sample_rate, SPEECH_RATE, "{mode}");
            assert_eq!(second.sample_rate, SPEECH_RATE, "{mode}");
            assert_eq!(first.samples, second.samples, "{mode}: flite reseed");
            assert!(
                first.samples.iter().any(|s| s.abs() > 1.0e-4),
                "{mode}: silence is not speech"
            );
            let (id, spoken) = stack.reported();
            assert_eq!(id, "system-flite", "{mode}");
            assert!(spoken.contains("NOT A21"), "{spoken}");
            eprintln!(
                "fallback_speak mode={mode} backend=system-flite voice={voice} samples={} rate={}",
                first.samples.len(),
                first.sample_rate
            );
        }
        "ABSENT" => {
            let reason = match &stack.backend {
                SpeechBackend::None { reason } => reason.clone(),
                other => panic!("{mode}: token ABSENT but {}", other.label()),
            };
            assert!(reason.contains("libflite"), "{mode}: {reason}");
            assert!(reason.contains("REFUSED"), "{mode}: {reason}");
            assert!(!stack.can_speak(), "{mode}");
            let err = stack.speak(PHRASE).expect_err("no backend means no speech");
            let msg = err.to_string();
            assert!(matches!(err, VoiceError::Backend(_)), "{mode}: {msg}");
            assert!(msg.contains("libflite"), "{mode}: {msg}");
            assert!(msg.contains("REFUSED"), "{mode}: {msg}");
            eprintln!("fallback_speak mode={mode} backend=none reason={reason}");
        }
        other => panic!("{mode}: selected {other} ({label})"),
    }
}

fn snapshot_weights() -> Vec<(String, Option<u64>)> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut roots = vec![
        manifest.join("../../weights"),
        manifest.join("weights"),
        PathBuf::from("weights"),
    ];
    roots.sort();
    roots.dedup();
    let mut out = Vec::new();
    for root in roots {
        snapshot_tree(&root, &mut out);
    }
    out
}

fn snapshot_tree(root: &Path, out: &mut Vec<(String, Option<u64>)>) {
    let label = root.display().to_string();
    let Ok(rd) = fs::read_dir(root) else {
        out.push((format!("{label}/ (absent)"), None));
        return;
    };
    out.push((format!("{label}/"), None));
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let path = ent.path();
        if path.is_dir() {
            snapshot_tree(&path, out);
        } else {
            let len = ent.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((path.display().to_string(), Some(len)));
        }
    }
}

fn socket_fds() -> Vec<String> {
    let Ok(rd) = fs::read_dir("/proc/self/fd") else {
        return Vec::new();
    };
    let mut sockets = Vec::new();
    for ent in rd.flatten() {
        let Ok(target) = fs::read_link(ent.path()) else {
            continue;
        };
        let text = target.to_string_lossy();
        if text.starts_with("socket:") {
            sockets.push(text.into_owned());
        }
    }
    sockets.sort();
    sockets
}

fn dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            saved: keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
