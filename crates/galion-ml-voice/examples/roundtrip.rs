// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! W20-20 canned English round-trip CLI.
//! ASR stub -> echo -> TTS stub -> 24 kHz mono PCM16 WAV.
//!
//! cargo run --offline -p galion-ml-voice --example roundtrip -- \
//!     [--out PATH.wav] [--lang en] [--text "hello galion"]
#[path = "common/demo.rs"]
mod demo;

fn main() {
    let mut out = String::from("target/galion-ml-voice/roundtrip-en.wav");
    let mut lang = String::from("en");
    let mut text = String::from(galion_ml_voice::stub::FIXTURE_EN_TEXT);
    let mut a = std::env::args().skip(1);
    while let Some(k) = a.next() {
        let v = a.next().unwrap_or_else(|| { eprintln!("missing value for {k}"); std::process::exit(64) });
        match k.as_str() {
            "--out" => out = v,
            "--lang" => lang = v,
            "--text" => text = v,
            _ => { eprintln!("unknown flag {k}"); std::process::exit(64) }
        }
    }
    let turn = match demo::round_trip(&text, &lang) {
        Ok(t) => t,
        Err(e) => { eprintln!("error: {e} (English-only prototype)"); std::process::exit(2) }
    };
    let wav = demo::wav_bytes(&turn.audio);
    if let Some(p) = std::path::Path::new(&out).parent() {
        if !p.as_os_str().is_empty() { std::fs::create_dir_all(p).expect("create output dir"); }
    }
    std::fs::write(&out, &wav).expect("write wav");
    let n = turn.audio.samples.len() as u64;
    println!("lang   : {}", turn.lang);
    println!("asr    : {}", turn.heard);
    println!("reply  : {}", turn.reply);
    println!("tts    : {n} samples @ {} Hz = {} ms", turn.audio.sample_rate, n * 1000 / turn.audio.sample_rate as u64);
    println!("wav    : {out} ({} bytes)", wav.len());
}
