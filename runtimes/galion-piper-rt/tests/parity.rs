// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Audio parity: galion-piper-rt (onnxruntime 1.31.1) against the legacy
//! piper 2023.11.14-2 CLI (onnxruntime 1.14.1) on the same pinned voice pack,
//! deterministic prosody (noise 0). Same sample count and correlation >= 0.999
//! per sentence. Needs `weights/` (scripts/a21/fetch-a21.sh +
//! scripts/a21/build-piper-rt.sh); set `GALION_REPO` when running from a
//! worktree without weights. Missing weights = skip with a message.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SENTENCES: &[&str] = &[
    "Yes.",
    "Oh, I see.",
    "I hear you, Mate.",
    "That sounds hard, tell me what happened next.",
    "Wait, are you sure about that?",
    "Okay! Let's try it again, slowly this time.",
    "The meeting moved to three thirty; bring the notes.",
    "Galion Voice runs fully local on this machine.",
];

fn weights() -> PathBuf {
    std::env::var("GALION_REPO").map(|r| Path::new(&r).join("weights"))
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights"))
}

fn run(bin: &Path, model: &Path, text: &str) -> Vec<i16> {
    let mut c = Command::new(bin)
        .args(["--model"]).arg(model)
        .args(["--output_raw", "--json-input", "--noise_scale", "0", "--noise_w", "0", "--length_scale", "1", "--sentence_silence", "0"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()));
    let line = format!("{}\n", serde_json::json!({ "text": text }));
    c.stdin.take().unwrap().write_all(line.as_bytes()).unwrap();
    let out = c.wait_with_output().unwrap();
    assert!(out.status.success(), "{} exited {:?}", bin.display(), out.status);
    let b = &out.stdout;
    (0..b.len() / 2).map(|i| i16::from_le_bytes([b[2 * i], b[2 * i + 1]])).collect()
}

fn corr(a: &[i16], b: &[i16]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let (ma, mb) = (a.iter().map(|&x| x as f64).sum::<f64>() / n, b.iter().map(|&x| x as f64).sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        let (x, y) = (x as f64 - ma, y as f64 - mb);
        sab += x * y; saa += x * x; sbb += y * y;
    }
    sab / (saa.sqrt() * sbb.sqrt()).max(1e-9)
}

#[test]
fn pcm_matches_legacy_piper() {
    let w = weights();
    let legacy = w.join("runtime/piper/piper");
    let model = w.join("tts/en_US-ljspeech-medium.onnx");
    if !legacy.is_file() || !model.is_file() {
        eprintln!("SKIP parity: {} or {} missing (fetch-a21.sh)", legacy.display(), model.display());
        return;
    }
    std::env::set_var("GALION_PIPER_RUNTIME_DIR", w.join("runtime"));
    let rt = PathBuf::from(env!("CARGO_BIN_EXE_galion-piper-rt"));
    for s in SENTENCES {
        let (a, b) = (run(&legacy, &model, s), run(&rt, &model, s));
        let c = corr(&a, &b);
        let maxd = a.iter().zip(&b).map(|(&x, &y)| (x as i32 - y as i32).abs()).max().unwrap_or(0);
        eprintln!("parity {:>5} vs {:>5} samples  corr {c:.6}  max|d| {maxd:>5}  {s:?}", a.len(), b.len());
        assert!(a.len() > 1000, "legacy piper returned no audio for {s:?}");
        assert_eq!(a.len(), b.len(), "sample count differs for {s:?}");
        assert!(c >= 0.999, "correlation {c} < 0.999 for {s:?}");
    }
}
