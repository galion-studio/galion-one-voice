// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! One real clone through the shipped slot: `CloneTts::try_new` (consent gate,
//! ref allowlist, weights, tokenizer), then one `synthesize`, written as a
//! 16-bit mono WAV. Prints one JSON line with the timings.
//!
//! ```text
//! GALION_CLONE_LOCAL_REFS=1 GALION_CLIP_STORE=/path/clip-store GALION_CLONE_PERSONA=samantha \
//!   cargo run --offline -p galion-ml-voice --features speech --example clone_probe -- out.wav "Text."
//! ```
//!
//! `--try-new-only` stops after the gate and prints its verdict.
//! CC0 voices also need the session's CC0 licence acceptance. The probe has
//! no browser, so `GALION_PROBE_CC0_ACCEPT=1` stands in for the ticked box
//! (session `probe`); without it a CC0 voice is refused at synthesize.

use galion_ml_voice::speech::clone_worker::CloneTts;
use galion_ml_voice::{LanguageId, TtsEngine};
use std::process::ExitCode;
use std::time::Instant;

fn write_wav(path: &str, rate: u32, samples: &[f32]) -> std::io::Result<()> {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
        .collect();
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    std::fs::write(path, out)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let persona = std::env::var("GALION_CLONE_PERSONA").unwrap_or_default();
    let t0 = Instant::now();
    let mut tts = match CloneTts::try_new() {
        Ok(t) => t,
        Err(e) => {
            println!("{{\"persona\":\"{persona}\",\"try_new\":\"refused\",\"reason\":{:?}}}", e.to_string());
            return ExitCode::from(2);
        }
    };
    let gate_ms = t0.elapsed().as_secs_f64() * 1e3;
    if args.first().map(String::as_str) == Some("--try-new-only") {
        println!("{{\"persona\":\"{persona}\",\"try_new\":\"ok\",\"model\":\"{}\",\"gate_ms\":{gate_ms:.1}}}", tts.model_name());
        return ExitCode::SUCCESS;
    }
    let (Some(out), Some(text)) = (args.first(), args.get(1)) else {
        eprintln!("usage: clone_probe OUT.wav TEXT | clone_probe --try-new-only");
        return ExitCode::from(64);
    };
    let cc0_accept = std::env::var("GALION_PROBE_CC0_ACCEPT").is_ok_and(|v| v == "1");
    let store = galion_consent::cc0_acceptances();
    store.set_active(Some("probe"));
    if cc0_accept {
        store.accept("probe");
    }
    let t1 = Instant::now();
    let audio = match tts.synthesize(text, LanguageId::En) {
        Ok(a) => a,
        Err(e) => {
            println!("{{\"persona\":\"{persona}\",\"cc0_accepted\":{cc0_accept},\"synth\":\"failed\",\"reason\":{:?}}}", e.to_string());
            return ExitCode::from(1);
        }
    };
    let synth_ms = t1.elapsed().as_secs_f64() * 1e3;
    let audio_ms = audio.samples.len() as f64 * 1e3 / audio.sample_rate.max(1) as f64;
    if let Err(e) = write_wav(out, audio.sample_rate, &audio.samples) {
        eprintln!("clone_probe: write {out}: {e}");
        return ExitCode::from(1);
    }
    println!(
        "{{\"persona\":\"{persona}\",\"cc0_accepted\":{cc0_accept},\"model\":\"{}\",\"gate_ms\":{gate_ms:.1},\"synth_ms\":{synth_ms:.1},\"audio_ms\":{audio_ms:.1},\"rtf\":{:.3},\"rate\":{},\"out\":{out:?}}}",
        tts.model_name(),
        synth_ms / audio_ms.max(1.0),
        audio.sample_rate
    );
    ExitCode::SUCCESS
}
