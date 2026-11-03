// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! One phone turn from a WAV file, or from a stub fixture string.
//!
//! ```text
//! GALION_ASR=interim cargo run -p galion-cascade --features cascade --example phone_turn -- [in.wav [out.wav]]
//! cargo run -p galion-cascade --features cascade --example phone_turn -- --fixture "hello"
//! ```
//!
//! Real speech is refused unless `GALION_ASR=interim` (placeholder, not recognition)
//! or a future ratified engine. No audio is written unless `out.wav` is given.
//! Exit 0 ok, 2 lane error, 64 bad args.

use galion_cascade::en_lane::wav_bytes;
use galion_cascade::{AsrMode, PhoneLane};
use galion_ml_voice::stub::fixture_audio;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut lane = PhoneLane::from_env();
    eprintln!(
        "phone lane asr={:?} llm={:?} speech={} ({})",
        lane.cfg.asr,
        lane.cfg.llm,
        lane.speech_backend().0,
        lane.speech_backend().1
    );
    let turn = if args.first().map(String::as_str) == Some("--fixture") {
        let text = args.get(1).map(String::as_str).unwrap_or("hello");
        let pcm = galion_cascade::en_lane::to_pcm16(&fixture_audio(text).samples);
        lane.turn(&pcm)
    } else if let Some(path) = args.first() {
        match std::fs::read(path) {
            Ok(b) => lane.turn_wav(&b),
            Err(e) => {
                eprintln!("read {path}: {e}");
                return ExitCode::from(64);
            }
        }
    } else {
        eprintln!("usage: phone_turn <in.wav> [out.wav] | phone_turn --fixture \"hello\"");
        return ExitCode::from(64);
    };
    let turn = match turn {
        Ok(t) => t,
        Err(e) => {
            eprintln!("phone error: {e}");
            if matches!(lane.cfg.asr, AsrMode::Stub | AsrMode::Off) {
                eprintln!("real speech needs an owner pick in docs/policy/RATIFY-EN-ASR-PHONE-PATH.md");
                eprintln!("GALION_ASR=interim is the labelled placeholder, not recognition");
            }
            return ExitCode::from(2);
        }
    };
    println!(
        "{{\"asr\":\"{}\",\"backend\":\"{}\",\"ms\":{},\"tts_first_pcm_ms\":{},\"wav_bytes\":{},\"recognised\":{}}}",
        turn.asr.id(),
        turn.speech_backend,
        turn.ms,
        turn.stage_ms.tts_first_pcm,
        turn.wav_bytes.len(),
        turn.asr.is_recognised()
    );
    if let Some(out) = args.get(1).filter(|_| args.first().map(String::as_str) != Some("--fixture")) {
        if let Err(e) = std::fs::write(out, &turn.wav_bytes) {
            eprintln!("write {out}: {e}");
            return ExitCode::from(64);
        }
        eprintln!("wrote {out}");
    } else if args.first().map(String::as_str) == Some("--fixture") {
        // Fixture path does not write unless a third arg is passed.
        if let Some(out) = args.get(2) {
            let bytes = if turn.wav_bytes.is_empty() { wav_bytes(&[], 24_000) } else { turn.wav_bytes.clone() };
            if let Err(e) = std::fs::write(out, bytes) {
                eprintln!("write {out}: {e}");
                return ExitCode::from(64);
            }
        }
    }
    ExitCode::SUCCESS
}
