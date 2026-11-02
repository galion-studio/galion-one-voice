// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! W21-03 EN cascade demo lane (live-demo hook).
//! ASR-stub -> route (pin > hint > LID) -> MockLlm reply -> TTS-stub -> WAV.
//! FP-03: the reply WAV is SPOKEN ENGLISH WORDS from the A21 neural TTS (Piper VITS
//! en_US-ljspeech-medium, CPU, MIT; scripts/a21/fetch-a21.sh) when PRESENT, else the
//! flite fallback. With no speech backend (SPEECH=ABSENT) the run fails (exit 3) unless
//! `--allow-tones 1` is given; tones are never written as the reply silently.
//! Prints `GALION_SPEECH=a21|flite|ABSENT` (also `speech` in the JSON) for gates/runbook.
//!
//! cargo run --offline -p galion-cascade --features cascade --example en_cascade -- \
//!     [--out-dir target/galion-demo] [--text "hello galion"] [--pin en] [--hint en] [--emotion neutral|happy|sad|angry] [--allow-tones 0|1]
//!
//! Writes (stable paths, consumed by scripts/run-en-cascade-demo.sh / live demo):
//!   <out-dir>/en-cascade.wav          reply as SPOKEN WORDS, 24 kHz mono PCM16, padded to 80 ms frames
//!   <out-dir>/en-cascade-speech.wav   same speech (explicit name)
//!   <out-dir>/en-cascade-earcon.wav   diagnostic earcon (tones, NOT the reply)
//!   <out-dir>/en-cascade-fixture.wav  exact StubTts signal (loop-check)
//!   <out-dir>/en-cascade.json         turn summary (+ speech_backend, a21_weights)
//! Exit: 0 ok, 2 unsupported language / empty input, 3 no speech backend, 64 bad flags.
use galion_cascade::en_lane::*;

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"") }

fn main() {
    let mut dir = String::from("target/galion-demo");
    let mut text = String::from("hello galion");
    let mut emotion = galion_cascade::Emotion::Neutral;
    let mut allow_tones = false;
    let (mut pin, mut hint): (Option<String>, Option<String>) = (None, None);
    let mut a = std::env::args().skip(1);
    while let Some(k) = a.next() {
        let v = a.next().unwrap_or_else(|| { eprintln!("missing value for {k}"); std::process::exit(64) });
        match k.as_str() {
            "--out-dir" => dir = v,
            "--text" => text = v,
            "--pin" => pin = Some(v),
            "--hint" => hint = Some(v),
            "--allow-tones" => allow_tones = v == "1" || v.eq_ignore_ascii_case("true"),
            "--emotion" => emotion = parse_emotion(&v).unwrap_or_else(|| { eprintln!("bad --emotion {v}"); std::process::exit(64) }),
            _ => { eprintln!("unknown flag {k}"); std::process::exit(64) }
        }
    }
    let d = match run_en_turn_with(&text, pin.as_deref(), hint.as_deref(), emotion) {
        Ok(d) => d,
        Err(e) => { eprintln!("error: {e:?} (English-only demo lane)"); std::process::exit(2) }
    };
    std::fs::create_dir_all(&dir).expect("create out dir");
    let p = std::path::Path::new(&dir);
    let reply_wav = p.join("en-cascade.wav");
    let speech_wav = p.join("en-cascade-speech.wav");
    let earcon_wav = p.join("en-cascade-earcon.wav");
    let fixture = p.join("en-cascade-fixture.wav");
    let json = p.join("en-cascade.json");
    std::fs::write(&earcon_wav, wav_bytes(&d.audible, LANE_RATE_HZ)).expect("write wav");
    std::fs::write(&fixture, wav_bytes(&d.fixture, LANE_RATE_HZ)).expect("write wav");
    // Reply render: speech, padded to whole 80 ms frames (LOCKS clock) for the live demo.
    let (reply_pcm, reply_kind) = match &d.speech {
        Some(sp) => {
            let mut v = sp.clone();
            let pad = (FRAME_SAMPLES - v.len() % FRAME_SAMPLES) % FRAME_SAMPLES;
            v.extend(std::iter::repeat(0).take(pad));
            std::fs::write(&speech_wav, wav_bytes(&v, LANE_RATE_HZ)).expect("write wav");
            (v, "speech")
        }
        None if allow_tones => { let _ = std::fs::remove_file(&speech_wav); (d.audible.clone(), "earcon-fallback-NOT-SPEECH") }
        None => {
            let _ = std::fs::remove_file(&reply_wav); let _ = std::fs::remove_file(&speech_wav);
            eprintln!("error: SPEECH=ABSENT — {}.", d.speech_backend.1);
            eprintln!("       FRXDEL Q2 requires spoken words. Fix: scripts/a21/fetch-a21.sh (A21 Piper, CPU) or apt install libflite1 (fallback);");
            eprintln!("       --allow-tones 1 forces the diagnostic earcon (NOT speech).");
            eprintln!("a21     : {}", d.a21_weights.1);
            println!("GALION_SPEECH=ABSENT");
            std::process::exit(3)
        }
    };
    std::fs::write(&reply_wav, wav_bytes(&reply_pcm, LANE_RATE_HZ)).expect("write wav");
    let token = match d.speech_backend.0 { "a21-weights" => "a21", "system-flite" => "flite", _ => "ABSENT" };
    let ms = |n: usize| n as u64 * 1000 / LANE_RATE_HZ as u64;
    let summary = format!(
        "{{\"lane\":\"en-cascade\",\"lang\":\"en\",\"route_source\":\"{:?}\",\"asr\":\"{}\",\"emotion\":\"{:?}\",\"reply\":\"{}\",\"spoken_text\":\"{}\",\"reply_kind\":\"{}\",\"speech\":\"{}\",\"speech_backend\":\"{}\",\"speech_backend_label\":\"{}\",\"a21_weights\":\"{}\",\"a21_weights_detail\":\"{}\",\"rate_hz\":{},\"frames\":{},\"audible_ms\":{},\"wav\":\"{}\",\"earcon_wav\":\"{}\",\"fixture_wav\":\"{}\"}}\n",
        d.route.source, esc(&d.turn.transcript.text), d.turn.emotion, esc(&d.turn.reply), esc(&d.spoken_text), reply_kind, token,
        d.speech_backend.0, esc(&d.speech_backend.1), d.a21_weights.0, esc(&d.a21_weights.1), LANE_RATE_HZ,
        reply_pcm.len() / FRAME_SAMPLES, ms(reply_pcm.len()), esc(&reply_wav.display().to_string()),
        esc(&earcon_wav.display().to_string()), esc(&fixture.display().to_string()));
    std::fs::write(&json, &summary).expect("write json");
    println!("lang    : en (route: {:?})", d.route.source);
    println!("asr     : {}", d.turn.transcript.text);
    println!("emotion : {:?}", d.turn.emotion);
    println!("reply   : {}", d.turn.reply);
    println!("spoken  : \"{}\" via {}", d.spoken_text, d.speech_backend.1);
    println!("a21     : {}", d.a21_weights.1);
    println!("wav     : {} [{}] ({} frames x 80 ms = {} ms @ {} Hz)", reply_wav.display(), reply_kind, reply_pcm.len() / FRAME_SAMPLES, ms(reply_pcm.len()), LANE_RATE_HZ);
    println!("earcon  : {} (diagnostic tones, not the reply)", earcon_wav.display());
    println!("fixture : {} ({} ms)", fixture.display(), ms(d.fixture.len()));
    println!("summary : {}", json.display());
    println!("GALION_SPEECH={token}");
}
