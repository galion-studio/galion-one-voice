// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2-S6 cold vs warm first-clause PCM for one Piper process.
//!
//! Does not pre-warm: the first `speak` is cold (process start, model load,
//! first clause) and the second is warm (same process, first clause again).
//! `PhoneLane::new` does pre-warm, so a caller turn is the warm path.
//! No PCM is written. Missing pack prints `measured: false` and no latency number.
//!
//! ```text
//! cargo run -q -p galion-ml-voice --features speech --example piper_warmth
//! ```

use galion_ml_voice::speech::{weights, SpeechStack};

fn main() {
    let status = weights::WeightStatus::discover();
    if weights::A21WeightTts::try_new(&status).is_err() {
        println!(
            "{{\"pack\":\"absent\",\"measured\":false,\"pcm_cache\":\"none\",\"note\":\"A21 pack or Piper runtime missing; no latency number invented\"}}"
        );
        return;
    }
    let mut stack = SpeechStack::detect();
    if stack.backend.speech_token() != "a21" {
        println!(
            "{{\"pack\":\"absent\",\"measured\":false,\"pcm_cache\":\"none\",\"note\":\"speech stack is not A21 ({})\"}}",
            stack.backend.speech_token()
        );
        return;
    }
    let voice = match &stack.backend {
        galion_ml_voice::speech::SpeechBackend::A21Weights { voice } => voice.clone(),
        _ => String::new(),
    };
    // First clause is "Hi!" so the clock stops before the rest of the sentence.
    let phrase = "Hi! This clause is the warm path check.";
    if let Err(e) = stack.speak(phrase) {
        eprintln!("cold speak failed: {e}");
        std::process::exit(1);
    }
    if let Err(e) = stack.speak(phrase) {
        eprintln!("warm speak failed: {e}");
        std::process::exit(1);
    }
    let w = stack.warmth();
    let (Some(cold), Some(warm)) = (w.cold_first_pcm_ms, w.warm_first_pcm_ms) else {
        eprintln!("warmth clock missing: {w:?}");
        std::process::exit(1);
    };
    let reused = w.piper_spawns == 1 && w.piper_pid.is_some();
    if !reused {
        eprintln!("worker was not reused: {w:?}");
        std::process::exit(1);
    }
    println!(
        "{{\"pack\":\"present\",\"measured\":true,\"voice\":\"{voice}\",\"cold_first_pcm_ms\":{cold},\"warm_first_pcm_ms\":{warm},\"piper_pid\":{},\"piper_spawns\":{},\"worker_reused\":true,\"pcm_cache\":\"none\",\"phrase\":\"Hi! This clause is the warm path check.\",\"clock\":\"speak entry until first clause PCM\"}}",
        w.piper_pid.unwrap_or(0),
        w.piper_spawns
    );
}
