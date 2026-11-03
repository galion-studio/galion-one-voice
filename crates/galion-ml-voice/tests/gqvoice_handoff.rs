// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! The real clone worker's stdout is not the runtime format.
//! Protocol-stub PCM is compiled into `.gqvoice` for Samantha and Jarvis.
#![cfg(feature = "speech")]

use galion_ml_voice::speech::clone_worker::{worker_script, CloneModel, CloneWorker};
use galion_ml_voice::speech::gqvoice::{self, GqVoice};
use std::path::PathBuf;

#[test]
fn chatterbox_worker_bytes_compile_before_playback() {
    std::env::set_var("GALION_CLONE_STUB_PCM", "1");
    let script = worker_script().expect("worker script");
    let dir = std::env::temp_dir().join(format!("galion-gqvoice-handoff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    land(&script, &dir, "samantha", CloneModel::Nano, "Hello.");
    land(
        &script,
        &dir,
        "jarvis",
        CloneModel::Turbo,
        "Systems are ready.",
    );

    let samantha = GqVoice::open(&dir.join("samantha.gqvoice")).unwrap();
    let jarvis = GqVoice::open(&dir.join("jarvis.gqvoice")).unwrap();
    assert_eq!(
        (samantha.persona_name(), samantha.model_name()),
        ("samantha", "nano")
    );
    assert_eq!(
        (jarvis.persona_name(), jarvis.model_name()),
        ("jarvis", "turbo")
    );
    assert_ne!(samantha.as_bytes(), jarvis.as_bytes());
    assert!(samantha.as_bytes().starts_with(b"GQVOICE\0"));
    assert!(jarvis.as_bytes().starts_with(b"GQVOICE\0"));
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn land(script: &std::path::Path, dir: &PathBuf, persona: &str, model: CloneModel, text: &str) {
    let mut worker = CloneWorker::spawn(script, model, persona).unwrap();
    let pcm = worker.synthesize(text).unwrap();
    assert!(!pcm.is_empty());
    let mut raw = Vec::new();
    for s in &pcm {
        raw.extend_from_slice(&s.to_le_bytes());
    }
    let refused = GqVoice::open_bytes(&raw).unwrap_err();
    assert!(refused.to_string().contains("raw chatterbox"), "{refused}");
    let audio = gqvoice::handoff_at(&pcm, persona, model.as_str(), dir).unwrap();
    let path = gqvoice::asset_path(dir, persona);
    let packet = GqVoice::open(&path).unwrap();
    assert_ne!(packet.as_bytes(), raw.as_slice());
    assert!(!packet.as_bytes().starts_with(b"RIFF"));
    assert_eq!(packet.playable(), pcm);
    assert_eq!(audio.samples, packet.to_audio().samples);
    assert_eq!(audio.sample_rate, 24_000);
    assert_eq!(packet.sector(0).unwrap().len(), 1920);
    drop(worker);
}
