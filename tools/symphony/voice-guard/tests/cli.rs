// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The `galion-voice-guard check` binary, on a synthetic tone.

use std::path::Path;
use std::process::Command;

fn sine(sr: u32, hz: f64, secs: f64, amp: f32) -> Vec<f32> {
    let n = (secs * sr as f64).round() as usize;
    (0..n)
        .map(|i| {
            let phase = 2.0 * std::f64::consts::PI * hz * i as f64 / sr as f64;
            (amp as f64 * phase.sin()) as f32
        })
        .collect()
}

fn write_wav(path: &Path, samples: &[f32], sr: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn cli_passes_a_tone_and_fails_a_nan_raw_buffer() {
    let dir = std::env::temp_dir().join(format!("galion-voice-guard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("tone.wav");
    let raw = dir.join("nan.f32");
    let out = dir.join("tone.json");
    write_wav(&wav, &sine(16_000, 180.0, 0.8, 0.2), 16_000);
    let mut samples = sine(16_000, 180.0, 0.8, 0.2);
    samples[400] = f32::NAN;
    let mut bytes = Vec::new();
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(&raw, bytes).unwrap();

    let bin = env!("CARGO_BIN_EXE_galion-voice-guard");
    let cfg =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/voice-guard.toml");
    let good = Command::new(bin)
        .args([
            "check",
            "--wav",
            wav.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(report["schema"], "galion-voice-guard/1");
    assert_eq!(report["verdict"], "pass");

    let bad = Command::new(bin)
        .args([
            "check",
            "--raw",
            raw.to_str().unwrap(),
            "--sr",
            "16000",
            "--config",
            cfg.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&bad.stdout).unwrap();
    assert_eq!(report["verdict"], "fail");
    assert!(report["reason"].as_str().unwrap().contains("finite"));
    let _ = std::fs::remove_dir_all(&dir);
}
