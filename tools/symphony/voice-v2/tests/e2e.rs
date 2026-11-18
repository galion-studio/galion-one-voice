// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic tones only. No voice audio, no weight download, no dataset.

use std::path::PathBuf;

use galion_voice_v2::audio::{vowel, write_wav};
use galion_voice_v2::decode::median_f0;
use galion_voice_v2::emotion::{EmotionId, ProsodyTier};
use galion_voice_v2::fluid::TargetState;
use galion_voice_v2::pipeline::{run_loop, speak, LoopIn, SpeakIn};
use galion_voice_v2::profile::VoiceFile;

fn voice() -> VoiceFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("voices/synthetic.toml");
    VoiceFile::load(&path).unwrap()
}

#[test]
fn reference_decodes_and_a_spoken_extended_emotion_corrects_once() {
    let voice = voice();
    assert_eq!(voice.id, "synthetic-a");
    assert!((voice.baseline.f0_hz_mean - 180.0).abs() < 1e-6);

    let dir = std::env::temp_dir().join(format!("galion-voice-v2-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("neutral.wav");
    let tone = vowel(180.0, 700.0, 1200.0, 1.6, 16000.0);
    write_wav(&wav, &tone, 16000).unwrap();

    let target = TargetState::spoken(EmotionId::Embarrassment, 1.0);
    let report = run_loop(&LoopIn {
        reference_wav: &wav,
        text: "A short spoken line.",
        target: &target,
        voice: &voice,
        out_dir: &dir,
        device: Some("cpu"),
    })
    .unwrap();

    assert_eq!(report.cleaner, "passthrough");
    assert_eq!(report.reference.tick_ms, 80);
    assert!(report.reference.n_ticks >= 16);
    assert_eq!(report.reference.nose.status, "stub");
    assert_eq!(report.reference.breath.status, "stub");
    assert!(report
        .reference
        .taxonomy
        .extended
        .iter()
        .any(|n| n == "embarrassment"));
    assert!(report
        .reference
        .taxonomy
        .reliable_in_prosody
        .iter()
        .any(|n| n == "anger"));
    assert!(!report
        .reference
        .taxonomy
        .reliable_in_prosody
        .iter()
        .any(|n| n == "embarrassment"));
    let ref_f0 = median_f0(&report.reference).expect("reference f0");
    assert!((ref_f0 - 180.0).abs() < 8.0, "reference f0 {ref_f0}");

    assert!(report.speak.write_ticks.iter().all(|t| {
        t.prosody == Some(EmotionId::Embarrassment)
            && t.burst.is_none()
            && t.prosody_tier == Some(ProsodyTier::Extended)
            && t.source == "plan"
    }));
    assert!(report
        .speak
        .readback
        .ticks
        .iter()
        .all(|t| t.burst.is_none() && t.source == "read"));
    assert!(report
        .speak
        .readback
        .ticks
        .iter()
        .any(|t| t.prosody.is_some()));
    assert_eq!(report.speak.device.cbx_ep, "cpu");
    assert!(matches!(
        report.speak.renderer,
        galion_voice_v2::render::RendererKind::Synthetic
    ));

    let intended = report.speak.diff_before.intended_f0_hz;
    assert!((report.speak.diff_after.intended_f0_hz - intended).abs() < 1e-6);
    let before = report
        .speak
        .diff_before
        .error_semitones
        .expect("first error");
    let after = report
        .speak
        .diff_after
        .error_semitones
        .expect("second error");
    assert!(
        after.abs() + 0.4 < before.abs(),
        "intended {intended} before {before} after {after}"
    );
    assert!(dir.join("profile.json").is_file());
    assert!(dir.join("readback.json").is_file());
    assert!(dir.join("corrected.wav").is_file());

    let sad = speak(&SpeakIn {
        text: "A quiet line.",
        target: &TargetState::spoken(EmotionId::Sadness, 1.0),
        voice: &voice,
        device: Some("cpu"),
    })
    .unwrap();
    let sad_f0 = sad.diff_before.measured_f0_hz.expect("sadness f0");
    assert!(sad_f0 < ref_f0 - 8.0, "sadness {sad_f0} reference {ref_f0}");
    assert!(sad
        .write_ticks
        .iter()
        .all(|t| t.prosody == Some(EmotionId::Sadness) && t.burst.is_none()));
    assert_eq!(sad.write_ticks[0].prosody_tier, Some(ProsodyTier::Reliable));

    let _ = std::fs::remove_dir_all(&dir);
}
