// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! One voice-to-voice session per persona, using the showcase beats.
//!
//! The words are the script in `docs/galion-voice-v2/V2V-SHOWCASE.md`.
//! They are text injected by this process. That is not Vosk and not full
//! voice-to-voice. The mouth is whatever `PhoneLane` actually speaks.
//! WAV files stay in `--wav-dir` (under `/tmp` or `/var/tmp`). This example
//! does not start the clone worker and does not set `GALION_CLONE_STUB_PCM`.
//!
//! ```text
//! cargo run -p galion-console --features phone-turn --example v2v_session -- --wav-dir /tmp/galion-v2v-sessions
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use galion_cascade::phone::FrameProsody;
use galion_cascade::PhoneLane;
use galion_console::v2v::{self, DuplexOutline};
use galion_emotion::EmotionFrame;
use galion_render::v2_persona;

const GRIEF: &str = "My mother died this morning.";
const JOY: &str = "I got the job. I can't stop smiling.";
const TASK: &str = "Confirm the gate for the six o'clock flight.";

struct Beat {
    name: &'static str,
    you: Option<&'static str>,
    sense: bool,
    caller: Option<EmotionFrame>,
    speak: bool,
    hub: Option<bool>,
}

fn main() {
    if std::env::var("GALION_CLONE_STUB_PCM").ok().as_deref() == Some("1") {
        eprintln!("v2v-session: GALION_CLONE_STUB_PCM=1 is protocol stub PCM. Refusing.");
        std::process::exit(2);
    }
    let wav_dir = wav_dir_from_args();
    fs::create_dir_all(&wav_dir).unwrap_or_else(|e| {
        eprintln!("v2v-session: mkdir {}: {e}", wav_dir.display());
        std::process::exit(2);
    });

    let mut lane = match PhoneLane::from_env() {
        Ok(lane) => lane,
        Err(e) => {
            eprintln!("v2v-session: phone lane: {e}");
            std::process::exit(2);
        }
    };
    let started = zagreb_now();
    let head = git_head();
    let host = command_out("hostname");
    let uname = command_out_args("uname", &["-srm"]);
    let (selected_id, selected_label) = lane.speech_backend();
    let header = Header {
        started,
        head,
        host,
        uname,
        selected_id: selected_id.to_string(),
        selected_label,
        can_speak: lane.can_speak(),
        phone_llm: format!("{:?}", lane.cfg.llm),
        phone_asr: format!("{:?}", lane.cfg.asr),
        speech_env: std::env::var("GALION_SPEECH").unwrap_or_else(|_| "unset (detect auto)".into()),
        llm_env: std::env::var("GALION_LLM").map(|v| format!("set:{v}")).unwrap_or_else(|_| "unset".into()),
        llm_url: if std::env::var_os("GALION_LLM_URL").is_some() { "set" } else { "unset" },
        vosk: Path::new("weights/asr/vosk-model-small-en-us-0.15").is_dir(),
        piper_pack: Path::new("weights/tts/en_US-ljspeech-medium.onnx").is_file(),
        samantha_wav: Path::new("docs/galion-voice-v2/personas/refs/samantha.wav").is_file(),
        jarvis_wav: Path::new("docs/galion-voice-v2/personas/refs/jarvis.wav").is_file(),
    };

    let samantha_beats = [
        Beat { name: "rest", you: Some(GRIEF), sense: false, caller: Some(tired()), speak: true, hub: None },
        Beat { name: "strain", you: Some(GRIEF), sense: true, caller: Some(tired()), speak: true, hub: None },
        Beat { name: "joy", you: Some(JOY), sense: true, caller: Some(joy_frame()), speak: true, hub: None },
    ];
    let jarvis_beats = [
        Beat { name: "command", you: Some("jarvis"), sense: true, caller: Some(tired()), speak: true, hub: None },
        Beat { name: "hub-on", you: None, sense: true, caller: None, speak: false, hub: Some(true) },
        Beat { name: "hub-off", you: None, sense: true, caller: None, speak: false, hub: Some(false) },
        Beat { name: "anger", you: Some(TASK), sense: true, caller: Some(angry()), speak: true, hub: None },
    ];

    let samantha = run_persona(&mut lane, "samantha", &samantha_beats, &wav_dir);
    let jarvis = run_persona(&mut lane, "jarvis", &jarvis_beats, &wav_dir);
    let doc_s = document(&header, "samantha", &samantha);
    let doc_j = document(&header, "jarvis", &jarvis);
    fs::write(wav_dir.join("samantha.json"), &doc_s).ok();
    fs::write(wav_dir.join("jarvis.json"), &doc_j).ok();
    println!("{doc_s}");
    println!("{doc_j}");
    if samantha.iter().chain(jarvis.iter()).any(|b| b.canned_empathy) {
        eprintln!("v2v-session: a canned empathy line was present");
        std::process::exit(1);
    }
}

struct Header {
    started: String,
    head: String,
    host: String,
    uname: String,
    selected_id: String,
    selected_label: String,
    can_speak: bool,
    phone_llm: String,
    phone_asr: String,
    speech_env: String,
    llm_env: String,
    llm_url: &'static str,
    vosk: bool,
    piper_pack: bool,
    samantha_wav: bool,
    jarvis_wav: bool,
}

struct BeatLog {
    name: &'static str,
    you: String,
    sense: bool,
    caller_applied: bool,
    caller: String,
    persona: String,
    reply: String,
    silence: bool,
    llm: String,
    hermes_attached: bool,
    inhouse: bool,
    mood: String,
    e: f32,
    oklch_l: f32,
    oklch_c: f32,
    oklch_h: f32,
    pitch_st: f32,
    pitch_range: f32,
    rate: f32,
    loudness: f32,
    warmth: f32,
    pre_pause_ms: u32,
    wash_amber: f32,
    film_voice: bool,
    clone_live: bool,
    canned_empathy: bool,
    nano_request_line: String,
    hub_enabled: bool,
    hub_note: String,
    speech_backend: String,
    speech_label: String,
    speech_error: String,
    wav_bytes: u64,
    sha256: String,
    sample_rate: u32,
    samples: usize,
    leading_zero_samples: usize,
    leading_zero_ms: String,
    body_rms: String,
    tts_first_pcm_ms: String,
    took_ms: u32,
}

fn run_persona(lane: &mut PhoneLane, persona: &str, beats: &[Beat], wav_dir: &Path) -> Vec<BeatLog> {
    let mut outline = DuplexOutline::new();
    let mut out = Vec::new();
    for beat in beats {
        if let Some(enabled) = beat.hub {
            if enabled {
                outline.set_hub(true);
            } else if persona == "jarvis" {
                match v2v::parse_toggle_line(r#"{"persona":"jarvis","hub_texture":false}"#) {
                    Ok(toggle) => outline.set_hub(toggle.enabled),
                    Err(e) => {
                        out.push(error_beat(beat, &outline, format!("hub toggle: {e}")));
                        continue;
                    }
                }
            }
        }
        outline.set_sense(beat.sense);
        let caller_applied = beat.sense && beat.caller.is_some();
        if beat.caller.is_some() {
            outline.on_heartbeat_frame(beat.caller.as_ref());
        }
        let plan = match beat.you {
            Some(text) => outline.on_asr_final(text),
            None => outline.plan(),
        };
        let canned = v2_persona::is_canned_empathy(&plan.text);
        let mut log = BeatLog {
            name: beat.name,
            you: beat.you.unwrap_or("").to_string(),
            sense: beat.sense,
            caller_applied,
            caller: caller_json(beat.caller.as_ref()),
            persona: plan.persona.to_string(),
            reply: plan.text.clone(),
            silence: plan.silence,
            llm: plan.llm.clone(),
            hermes_attached: plan.hermes_attached,
            inhouse: plan.inhouse,
            mood: plan.mood.clone(),
            e: plan.e,
            oklch_l: plan.oklch_l,
            oklch_c: plan.oklch_c,
            oklch_h: plan.oklch_h,
            pitch_st: plan.pitch_st,
            pitch_range: plan.pitch_range,
            rate: plan.rate,
            loudness: plan.loudness,
            warmth: plan.warmth,
            pre_pause_ms: plan.pre_pause_ms,
            wash_amber: plan.wash_amber,
            film_voice: plan.film_voice,
            clone_live: plan.clone_live,
            canned_empathy: canned,
            nano_request_line: outline.nano_request_line(),
            hub_enabled: outline.hub().enabled,
            hub_note: hub_note(&outline),
            speech_backend: String::new(),
            speech_label: String::new(),
            speech_error: String::new(),
            wav_bytes: 0,
            sha256: String::new(),
            sample_rate: 0,
            samples: 0,
            leading_zero_samples: 0,
            leading_zero_ms: "not measured".into(),
            body_rms: "not measured".into(),
            tts_first_pcm_ms: "not measured".into(),
            took_ms: 0,
        };
        if beat.speak && !plan.silence && !plan.text.is_empty() && !canned {
            lane.set_delivery(plan.persona, frame_prosody(&plan));
            let t0 = Instant::now();
            match lane.speak_fixed(&plan.text) {
                Ok(turn) => {
                    log.took_ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
                    log.speech_backend = turn.speech_backend.to_string();
                    log.speech_label = turn.speech_label.clone();
                    log.tts_first_pcm_ms = turn.stage_ms.tts_first_pcm.to_string();
                    log.wav_bytes = turn.wav_bytes.len() as u64;
                    if let Some(pcm) = pcm16(&turn.wav_bytes) {
                        log.sample_rate = wav_rate(&turn.wav_bytes);
                        log.samples = pcm.len();
                        log.leading_zero_samples = pcm.iter().take_while(|s| **s == 0).count();
                        if log.sample_rate > 0 {
                            let ms = log.leading_zero_samples as f64 * 1000.0 / f64::from(log.sample_rate);
                            log.leading_zero_ms = format!("{ms:.1}");
                        }
                        log.body_rms = format!("{:.6}", body_rms(&pcm));
                    }
                    let path = wav_dir.join(format!("{persona}-{}.wav", beat.name));
                    match fs::write(&path, &turn.wav_bytes) {
                        Ok(()) => log.sha256 = sha256(&path),
                        Err(e) => log.speech_error = format!("wav write: {e}"),
                    }
                    if turn.speech_backend == "a21-weights" {
                        outline.set_worker_engine("piper");
                    }
                }
                Err(e) => {
                    log.took_ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
                    log.speech_error = e.to_string();
                    let (id, label) = lane.speech_backend();
                    log.speech_backend = id.to_string();
                    log.speech_label = label;
                }
            }
        } else if !beat.speak {
            log.speech_error = "not spoken on this beat".into();
        } else if plan.silence || plan.text.is_empty() {
            log.speech_error = "floor returned silence".into();
        }
        out.push(log);
    }
    out
}

fn error_beat(beat: &Beat, outline: &DuplexOutline, err: String) -> BeatLog {
    let plan = outline.plan();
    BeatLog {
        name: beat.name,
        you: String::new(),
        sense: beat.sense,
        caller_applied: false,
        caller: "null".into(),
        persona: plan.persona.to_string(),
        reply: String::new(),
        silence: true,
        llm: plan.llm,
        hermes_attached: plan.hermes_attached,
        inhouse: plan.inhouse,
        mood: plan.mood,
        e: plan.e,
        oklch_l: plan.oklch_l,
        oklch_c: plan.oklch_c,
        oklch_h: plan.oklch_h,
        pitch_st: plan.pitch_st,
        pitch_range: plan.pitch_range,
        rate: plan.rate,
        loudness: plan.loudness,
        warmth: plan.warmth,
        pre_pause_ms: plan.pre_pause_ms,
        wash_amber: plan.wash_amber,
        film_voice: plan.film_voice,
        clone_live: plan.clone_live,
        canned_empathy: false,
        nano_request_line: outline.nano_request_line(),
        hub_enabled: outline.hub().enabled,
        hub_note: err.clone(),
        speech_backend: String::new(),
        speech_label: String::new(),
        speech_error: err,
        wav_bytes: 0,
        sha256: String::new(),
        sample_rate: 0,
        samples: 0,
        leading_zero_samples: 0,
        leading_zero_ms: "not measured".into(),
        body_rms: "not measured".into(),
        tts_first_pcm_ms: "not measured".into(),
        took_ms: 0,
    }
}

fn frame_prosody(plan: &galion_console::v2v::SpeakPlan) -> FrameProsody {
    FrameProsody {
        pitch_st: plan.pitch_st,
        pitch_range: plan.pitch_range,
        rate: plan.rate,
        energy: plan.loudness,
        warmth: plan.warmth,
        pre_pause_ms: u16::try_from(plan.pre_pause_ms).unwrap_or(u16::MAX),
    }
}

fn hub_note(outline: &DuplexOutline) -> String {
    let asset = [0.10_f32, -0.20, 0.30];
    let layer = [0.01_f32, 0.50, -0.10];
    match outline.playback(&asset, 24_000, &layer, 24_000) {
        Ok(mixed) => {
            let nums: Vec<String> = mixed.iter().map(|n| num(*n)).collect();
            format!(
                "unit-buffer-not-a-recording samples=[{}] hub_enabled={}",
                nums.join(","),
                outline.hub().enabled
            )
        }
        Err(e) => format!("unit mix refused: {e}"),
    }
}

fn document(header: &Header, persona: &str, beats: &[BeatLog]) -> String {
    let mut body = String::new();
    for (i, beat) in beats.iter().enumerate() {
        if i > 0 {
            body.push_str(",\n");
        }
        body.push_str(&beat_json(beat));
    }
    let refs_present = header.samantha_wav && header.jarvis_wav;
    format!(
        r#"{{
  "schema": "galion-v2v-session/1",
  "persona_session": {persona},
  "started": {started},
  "git_head": {head},
  "host": {host},
  "uname": {uname},
  "asr": "text-inject",
  "asr_note": "Showcase lines were typed by this process. Vosk was not used. This is not full voice-to-voice.",
  "vosk_model_present": {vosk},
  "full_v2v": false,
  "piper_pack_present": {piper},
  "selected_speech_backend": {selected_id},
  "selected_speech_label": {selected_label},
  "can_speak": {can_speak},
  "phone_lane_llm": {phone_llm},
  "phone_lane_asr_mode": {phone_asr},
  "phone_lane_llm_note": "speak_fixed does not ask the phone-lane LLM for words. The reply text is the persona floor.",
  "GALION_SPEECH": {speech_env},
  "GALION_LLM": {llm_env},
  "GALION_LLM_URL": {llm_url},
  "original_ref_wavs_present": {refs},
  "c5": "BLOCKED",
  "c5_clone_generate": "not run",
  "c5_reason": "Original Samantha and Jarvis wavs are not on this machine, so the clone worker was not started. A0 remains BLOCKED-OWNER. No clone PCM was produced here.",
  "mouth_is_clone_of_originals": false,
  "product_ttfa_claim": false,
  "listened_by_ear": false,
  "listen_note": "No person heard these files in this run. Numbers below are measurements of the WAV this process wrote. Pitch was not measured.",
  "beats": [
{body}
  ]
}}"#,
        persona = js(persona),
        started = js(&header.started),
        head = js(&header.head),
        host = js(&header.host),
        uname = js(&header.uname),
        vosk = header.vosk,
        piper = header.piper_pack,
        selected_id = js(&header.selected_id),
        selected_label = js(&header.selected_label),
        can_speak = header.can_speak,
        phone_llm = js(&header.phone_llm),
        phone_asr = js(&header.phone_asr),
        speech_env = js(&header.speech_env),
        llm_env = js(&header.llm_env),
        llm_url = js(header.llm_url),
        refs = refs_present,
        body = body,
    )
}

fn beat_json(b: &BeatLog) -> String {
    format!(
        r#"    {{
      "beat": {name},
      "you": {you},
      "sense": {sense},
      "caller_applied": {applied},
      "caller": {caller},
      "persona": {persona},
      "reply": {reply},
      "silence": {silence},
      "llm": {llm},
      "hermes_attached": {hermes},
      "inhouse": {inhouse},
      "mood": {mood},
      "e": {e},
      "oklch": [{l}, {c}, {h}],
      "pitch_st": {pitch},
      "pitch_range": {pr},
      "rate": {rate},
      "loudness": {loud},
      "warmth": {warmth},
      "pre_pause_ms": {pause},
      "wash_amber": {wash},
      "film_voice": {film},
      "clone_live": {clone},
      "canned_empathy": {canned},
      "nano_request_line_not_sent": {nano},
      "hub_enabled": {hub},
      "hub": {hub_note},
      "speech_backend": {backend},
      "speech_label": {label},
      "speech_error": {err},
      "wav_bytes": {wav_bytes},
      "sha256": {sha},
      "sample_rate_hz": {rate_hz},
      "samples": {samples},
      "leading_zero_samples": {zeros},
      "leading_zero_ms": {zero_ms},
      "body_rms": {rms},
      "tts_first_pcm_ms": {ttfa},
      "took_ms": {took}
    }}"#,
        name = js(b.name),
        you = js(&b.you),
        sense = b.sense,
        applied = b.caller_applied,
        caller = b.caller,
        persona = js(&b.persona),
        reply = js(&b.reply),
        silence = b.silence,
        llm = js(&b.llm),
        hermes = b.hermes_attached,
        inhouse = b.inhouse,
        mood = js(&b.mood),
        e = num(b.e),
        l = num(b.oklch_l),
        c = num(b.oklch_c),
        h = num(b.oklch_h),
        pitch = num(b.pitch_st),
        pr = num(b.pitch_range),
        rate = num(b.rate),
        loud = num(b.loudness),
        warmth = num(b.warmth),
        pause = b.pre_pause_ms,
        wash = num(b.wash_amber),
        film = b.film_voice,
        clone = b.clone_live,
        canned = b.canned_empathy,
        nano = js(&b.nano_request_line),
        hub = b.hub_enabled,
        hub_note = js(&b.hub_note),
        backend = js(&b.speech_backend),
        label = js(&b.speech_label),
        err = js(&b.speech_error),
        wav_bytes = b.wav_bytes,
        sha = js(&b.sha256),
        rate_hz = b.sample_rate,
        samples = b.samples,
        zeros = b.leading_zero_samples,
        zero_ms = js(&b.leading_zero_ms),
        rms = js(&b.body_rms),
        ttfa = js(&b.tts_first_pcm_ms),
        took = b.took_ms,
    )
}

fn caller_json(frame: Option<&EmotionFrame>) -> String {
    let Some(f) = frame else {
        return "null".into();
    };
    format!(
        "{{\"valence\":{},\"arousal\":{},\"dominance\":{},\"confidence\":{}}}",
        num(f.valence),
        num(f.arousal),
        num(f.dominance),
        num(f.confidence)
    )
}

fn tired() -> EmotionFrame {
    EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: -0.6, arousal: 0.2, dominance: 0.4, confidence: 1.0 }
}

fn joy_frame() -> EmotionFrame {
    EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: 0.8, arousal: 0.8, dominance: 0.5, confidence: 0.9 }
}

fn angry() -> EmotionFrame {
    EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: -0.8, arousal: 0.9, dominance: 0.8, confidence: 1.0 }
}

fn pcm16(wav: &[u8]) -> Option<Vec<i16>> {
    if wav.len() < 44 || &wav[..4] != b"RIFF" {
        return None;
    }
    Some(wav[44..].chunks(2).filter(|c| c.len() == 2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
}

fn wav_rate(wav: &[u8]) -> u32 {
    if wav.len() < 28 {
        return 0;
    }
    u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]])
}

fn body_rms(pcm: &[i16]) -> f64 {
    let body: Vec<i16> = pcm.iter().copied().skip_while(|s| *s == 0).collect();
    if body.is_empty() {
        return 0.0;
    }
    let sum: f64 = body.iter().map(|s| {
        let x = f64::from(*s) / 32768.0;
        x * x
    }).sum();
    (sum / body.len() as f64).sqrt()
}

fn sha256(path: &Path) -> String {
    let out = Command::new("sha256sum").arg(path).output();
    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or("").to_string()
        }
        _ => String::new(),
    }
}

fn num(x: f32) -> String {
    if !x.is_finite() {
        return "null".into();
    }
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn wav_dir_from_args() -> PathBuf {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args.iter().position(|a| a == "--wav-dir").and_then(|i| args.get(i + 1)).cloned();
    let Some(dir) = dir else {
        eprintln!("usage: v2v_session --wav-dir /tmp/galion-v2v-sessions");
        std::process::exit(64);
    };
    if !(dir.starts_with("/tmp/") || dir.starts_with("/var/tmp/")) {
        eprintln!("v2v-session: wav dir must be under /tmp or /var/tmp");
        std::process::exit(2);
    }
    PathBuf::from(dir)
}

fn zagreb_now() -> String {
    let out = Command::new("date").arg("+%Y-%m-%dT%H:%M:%S%z").env("TZ", "Europe/Zagreb").output();
    match out {
        Ok(out) if out.status.success() => {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if raw.len() >= 5 && (raw.ends_with("00") || raw.contains('+') || raw.contains('-')) {
                let (body, off) = raw.split_at(raw.len() - 2);
                format!("{body}:{off}")
            } else {
                raw
            }
        }
        _ => "not measured".into(),
    }
}

fn git_head() -> String {
    command_out_args("git", &["rev-parse", "HEAD"])
}

fn command_out(cmd: &str) -> String {
    command_out_args(cmd, &[])
}

fn command_out_args(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => String::new(),
    }
}
