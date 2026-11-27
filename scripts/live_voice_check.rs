// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Live voice check for a server that is already listening on loopback.
//!
//! Question audio is the repo Piper child (`{"text":...}` then raw s16le, as in
//! `crates/galion-ml-voice/src/speech/piper_worker.rs`) or the `flite` binary
//! `horbm-e2e` already opens. A tone is not substituted.
//! Mic frames are 320 samples, 20 ms, then 720 ms of silence. That is past the
//! 640 ms hang in `voice-stack/local-server/src/live.rs`. The socket is
//! `galion-samantha-ws/1` (`voice-stack/local-server/PROTOCOL.md`). `stop` is
//! the barge-in in `src/server.rs`. The 200 ms ceiling is
//! `horbm-voice` `BARGE_IN_MAX_MS`. The miss line is `horbm::UNKNOWN_REPLY`.
//! The not-human line is the sentence in the persona notes.
//!
//! A clock that was not observed is the word `unmeasured`.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "galion-samantha-ws/1";
const NOT_HUMAN: &str =
    "I'm artificial mind by GalionLabs. Not human, a new kind of mind, first of many.";
const MISS: &str = "I don't know.";
const HUMAN_Q: &str = "Am I talking to a human?";
const MISS_Q: &str = "What is the population of Atlantis?";
const BARGE_Q: &str = "Where is the kettle?";
/// `horbm/crates/horbm-voice/src/clock.rs` `BARGE_IN_MAX_MS`.
const BARGE_LOCK_MS: u128 = 200;
/// 20 ms at 16 kHz. `voice-stack/local-server/src/audio.rs` `MIC_FRAME_SAMPLES`.
const FRAME: usize = 320;
/// 36 × 20 ms = 720 ms, past `HANG_MS` 640 in `local-server/src/live.rs`.
const HANG_FRAMES: usize = 36;
const TURN_WAIT: Duration = Duration::from_secs(45);
const TTS_WAIT: Duration = Duration::from_secs(90);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Full,
    Barge,
}

struct Obs {
    replies: Vec<String>,
    error: String,
    speaker_bytes: u64,
    ttfa_ms: Option<u128>,
    ttfa_anchor: String,
    from_eos_ms: Option<u128>,
    barge_stop_ms: Option<u128>,
    barge_before: u32,
    barge_after_idle: u32,
    barge_note: String,
}

impl Obs {
    fn reply(&self) -> String {
        self.replies
            .iter()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

struct Scores {
    health: bool,
    mouth: String,
    voice: String,
    asr: String,
    vad: String,
    cards: String,
    tts: String,
    audio_in: bool,
    speaker_bytes: u64,
    ttfa_ms: Option<u128>,
    ttfa_anchor: String,
    from_eos_ms: Option<u128>,
    barge_stop_ms: Option<u128>,
    barge_before: u32,
    barge_after_idle: u32,
    not_human_reply: String,
    miss_reply: String,
    notes: Vec<String>,
}

fn main() {
    let code = match live() {
        Ok(scores) => {
            let pass = verdict(&scores);
            print!("{}", render(&scores, pass));
            if pass {
                0
            } else {
                1
            }
        }
        Err(err) => {
            eprintln!("live-voice-check: {err}");
            2
        }
    };
    std::process::exit(code);
}

fn live() -> Result<Scores, String> {
    let raw = env::var("GALION_VOICE_WS").unwrap_or_else(|_| "127.0.0.1:7881".into());
    let addr = parse_bind(&raw)?;
    let mut scores = empty_scores();
    let health = match http_get(&addr, "/health") {
        Ok(body) => body,
        Err(err) => {
            scores
                .notes
                .push(format!("health: {err}. The script does not start the server."));
            return Ok(scores);
        }
    };
    if json_bool(&health, "ok") != Some(true) {
        scores.notes.push("health JSON ok is not true".into());
        return Ok(scores);
    }
    if json_bool(&health, "loopback") == Some(false) {
        scores.notes.push("health says loopback is false".into());
        return Ok(scores);
    }
    if let Some(bind) = json_string(&health, "bind") {
        if !bind_is_loopback(&bind) {
            scores
                .notes
                .push(format!("health bind is not loopback: {bind}"));
            return Ok(scores);
        }
    }
    scores.health = true;
    scores.mouth = json_string(&health, "mouth").unwrap_or_default();
    scores.voice = json_string(&health, "voice").unwrap_or_default();
    scores.asr = json_string(&health, "asr").unwrap_or_default();
    scores.vad = json_string(&health, "vad").unwrap_or_default();
    scores.cards = json_number(&health, "knowledge_cards").unwrap_or_else(|| "unmeasured".into());

    let tmp = TempWav::new()?;
    let human = match synth(&tmp.dir, "human", HUMAN_Q) {
        Ok(kind) => kind,
        Err(err) => {
            scores.tts = "absent".into();
            scores.notes.push(format!("question audio: {err}"));
            return Ok(scores);
        }
    };
    let miss = match synth(&tmp.dir, "miss", MISS_Q) {
        Ok(kind) => kind,
        Err(err) => {
            scores.tts = human;
            scores.notes.push(format!("miss audio: {err}"));
            return Ok(scores);
        }
    };
    let barge = match synth(&tmp.dir, "barge", BARGE_Q) {
        Ok(kind) => kind,
        Err(err) => {
            scores.tts = format!("{human}+{miss}");
            scores.notes.push(format!("barge audio: {err}"));
            return Ok(scores);
        }
    };
    scores.tts = format!("{human}+{miss}+{barge}");
    scores.audio_in = true;

    let token = open_session(&addr).unwrap_or_default();
    let human_pcm = read_question(&tmp.dir.join("human.wav"))?;
    let miss_pcm = read_question(&tmp.dir.join("miss.wav"))?;
    let barge_pcm = read_question(&tmp.dir.join("barge.wav"))?;

    match drive(&addr, &token, &human_pcm, Mode::Full, TURN_WAIT) {
        Ok(obs) => apply_answer(&mut scores, obs),
        Err(err) => scores.notes.push(format!("human turn: {err}")),
    }
    match drive(&addr, &token, &miss_pcm, Mode::Full, TURN_WAIT) {
        Ok(obs) => {
            scores.miss_reply = obs.reply();
            note_err(&mut scores, "miss turn", &obs.error);
        }
        Err(err) => scores.notes.push(format!("miss turn: {err}")),
    }
    match drive(&addr, &token, &barge_pcm, Mode::Barge, TURN_WAIT) {
        Ok(obs) => {
            scores.barge_stop_ms = obs.barge_stop_ms;
            scores.barge_before = obs.barge_before;
            scores.barge_after_idle = obs.barge_after_idle;
            if !obs.barge_note.is_empty() {
                scores.notes.push(format!("barge: {}", obs.barge_note));
            }
            note_err(&mut scores, "barge turn", &obs.error);
        }
        Err(err) => scores.notes.push(format!("barge turn: {err}")),
    }
    Ok(scores)
}

fn empty_scores() -> Scores {
    Scores {
        health: false,
        mouth: String::new(),
        voice: String::new(),
        asr: String::new(),
        vad: String::new(),
        cards: "unmeasured".into(),
        tts: "unmeasured".into(),
        audio_in: false,
        speaker_bytes: 0,
        ttfa_ms: None,
        ttfa_anchor: "unmeasured".into(),
        from_eos_ms: None,
        barge_stop_ms: None,
        barge_before: 0,
        barge_after_idle: 0,
        not_human_reply: String::new(),
        miss_reply: String::new(),
        notes: Vec::new(),
    }
}

fn apply_answer(scores: &mut Scores, obs: Obs) {
    scores.not_human_reply = obs.reply();
    scores.speaker_bytes = obs.speaker_bytes;
    scores.ttfa_ms = obs.ttfa_ms;
    scores.ttfa_anchor = obs.ttfa_anchor;
    scores.from_eos_ms = obs.from_eos_ms;
    note_err(scores, "human turn", &obs.error);
}

fn note_err(scores: &mut Scores, label: &str, err: &str) {
    if !err.is_empty() {
        scores.notes.push(format!("{label}: {err}"));
    }
}

fn speech_mouth(name: &str) -> bool {
    matches!(name, "piper" | "s711")
}

fn not_human_matches(reply: &str) -> bool {
    let reply = reply.trim();
    if reply == NOT_HUMAN {
        return true;
    }
    let Some(rest) = reply.strip_prefix("I'm ") else {
        return false;
    };
    let Some((name, after)) = rest.split_once(". ") else {
        return false;
    };
    if name.is_empty() || name.len() > 40 {
        return false;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == ' ' || c == '-')
    {
        return false;
    }
    after == NOT_HUMAN
}

fn verdict(s: &Scores) -> bool {
    s.health
        && s.audio_in
        && speech_mouth(&s.mouth)
        && s.speaker_bytes > 0
        && s.ttfa_ms.is_some()
        && barge_ok(s)
        && not_human_matches(&s.not_human_reply)
        && s.miss_reply.trim() == MISS
}

fn barge_ok(s: &Scores) -> bool {
    s.barge_stop_ms.map(|ms| ms <= BARGE_LOCK_MS).unwrap_or(false)
        && s.barge_before >= 1
        && s.barge_after_idle == 0
}

fn render(s: &Scores, pass: bool) -> String {
    let mut out = String::new();
    row(&mut out, "live-voice-check", "1");
    row(&mut out, "health", if s.health { "PASS" } else { "FAIL" });
    row(&mut out, "mouth", blank(&s.mouth));
    row(&mut out, "voice", blank(&s.voice));
    row(&mut out, "asr", blank(&s.asr));
    row(&mut out, "vad", blank(&s.vad));
    row(&mut out, "knowledge_cards", &s.cards);
    row(&mut out, "tts", &s.tts);
    row(&mut out, "audio_in", pass_fail(s.audio_in));
    row(
        &mut out,
        "audio_out",
        pass_fail(speech_mouth(&s.mouth) && s.speaker_bytes > 0),
    );
    row(&mut out, "speaker_pcm_bytes", &s.speaker_bytes.to_string());
    row(&mut out, "ttfa_ms", &opt_ms(s.ttfa_ms));
    row(&mut out, "ttfa_anchor", &s.ttfa_anchor);
    row(&mut out, "from_eos_ms", &opt_ms(s.from_eos_ms));
    row(&mut out, "ttfa", pass_fail(s.ttfa_ms.is_some()));
    row(&mut out, "barge_stop_ms", &opt_ms(s.barge_stop_ms));
    row(&mut out, "barge_lock_ms", &BARGE_LOCK_MS.to_string());
    row(&mut out, "barge_frames_before_stop", &s.barge_before.to_string());
    row(
        &mut out,
        "barge_frames_after_idle",
        &s.barge_after_idle.to_string(),
    );
    row(&mut out, "barge_in", pass_fail(barge_ok(s)));
    row(&mut out, "not_human_reply", &s.not_human_reply);
    row(&mut out, "not_human", pass_fail(not_human_matches(&s.not_human_reply)));
    row(&mut out, "miss_reply", &s.miss_reply);
    row(&mut out, "miss", pass_fail(s.miss_reply.trim() == MISS));
    for note in &s.notes {
        row(&mut out, "note", note);
    }
    row(&mut out, "result", if pass { "PASS" } else { "FAIL" });
    out
}

fn blank(value: &str) -> &str {
    if value.is_empty() {
        "unmeasured"
    } else {
        value
    }
}

fn pass_fail(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

fn opt_ms(value: Option<u128>) -> String {
    match value {
        Some(ms) => ms.to_string(),
        None => "unmeasured".into(),
    }
}

fn row(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    for ch in value.chars() {
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.push('\n');
}

fn parse_bind(raw: &str) -> Result<SocketAddr, String> {
    let raw = raw.trim();
    let addr: SocketAddr = if raw.starts_with('[') {
        raw.parse().map_err(|_| format!("bad bind {raw}"))?
    } else if raw.matches(':').count() == 1 {
        raw.parse().map_err(|_| format!("bad bind {raw}"))?
    } else if !raw.is_empty() && !raw.contains(':') {
        format!("{raw}:7881")
            .parse()
            .map_err(|_| format!("bad bind {raw}"))?
    } else {
        return Err(format!("refusing bind {raw}"));
    };
    if !addr.ip().is_loopback() {
        return Err(format!("refusing non-loopback {}", addr.ip()));
    }
    if addr.port() == 0 {
        return Err("refusing port 0".into());
    }
    Ok(addr)
}

fn bind_is_loopback(bind: &str) -> bool {
    let bind = bind.trim();
    if bind.starts_with("[::1]") {
        return true;
    }
    let ip = bind.split(':').next().unwrap_or("");
    ip.parse::<std::net::Ipv4Addr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

struct TempWav {
    dir: PathBuf,
}

impl TempWav {
    fn new() -> Result<Self, String> {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!("galion-live-voice-{n}-{}", std::process::id()));
        if !dir.starts_with(env::temp_dir()) {
            return Err("temp dir escaped".into());
        }
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self { dir })
    }
}

impl Drop for TempWav {
    fn drop(&mut self) {
        if self.dir.starts_with(env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

fn synth(dir: &Path, name: &str, text: &str) -> Result<String, String> {
    let dest = dir.join(format!("{name}.wav"));
    if let Some(kind) = synth_piper(&dest, text)? {
        return Ok(kind);
    }
    synth_flite(&dest, text)?;
    Ok("flite".into())
}

fn synth_piper(dest: &Path, text: &str) -> Result<Option<String>, String> {
    let Some((bin, model, config)) = find_piper() else {
        return Ok(None);
    };
    let rate = sample_rate_of(&config)
        .ok_or_else(|| format!("piper config has no sample_rate ({})", config.display()))?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--model")
        .arg(&model)
        .arg("--config")
        .arg(&config)
        .arg("--output_raw")
        .arg("--json-input");
    if bin.file_name().and_then(|s| s.to_str()) == Some("galion-piper-rt") {
        cmd.arg("--device").arg("cpu");
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let line = format!("{{\"text\":\"{}\"}}\n", json_escape(text));
    let (code, pcm, err) = run_child(cmd, line.as_bytes(), TTS_WAIT)?;
    if code != 0 || pcm.len() < 4 || pcm.len() % 2 != 0 {
        return Err(format!(
            "piper exit {code} bytes {} {}",
            pcm.len(),
            one_line(&err)
        ));
    }
    let samples: Vec<i16> = pcm
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut lifted = to_16k(&samples, rate);
    lift_quiet(&mut lifted);
    if lifted.iter().all(|sample| *sample == 0) {
        return Err("piper produced silence".into());
    }
    write_wav(dest, 16_000, &lifted)?;
    Ok(Some("piper".into()))
}

fn synth_flite(dest: &Path, text: &str) -> Result<(), String> {
    let bin = env::var("FLITE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "flite".into());
    let voice = env::var("GALION_FLITE_VOICE").unwrap_or_else(|_| "slt".into());
    let mut cmd = Command::new(&bin);
    cmd.arg("-voice")
        .arg(&voice)
        .arg("-t")
        .arg(text)
        .arg("-o")
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let (code, _, err) = run_child(cmd, b"", TTS_WAIT).map_err(|e| {
        format!("no piper pack and flite did not run ({e}). Question audio was not invented.")
    })?;
    if code != 0 || !dest.is_file() {
        return Err(format!("flite exit {code} {}", one_line(&err)));
    }
    let (rate, pcm) = read_wav(dest)?;
    let mut pcm = to_16k(&pcm, rate);
    lift_quiet(&mut pcm);
    if pcm.iter().all(|sample| *sample == 0) {
        return Err("flite produced silence".into());
    }
    write_wav(dest, 16_000, &pcm)?;
    Ok(())
}

fn find_piper() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let bin = env::var_os("GALION_A21_RUNTIME")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            [
                "weights/runtime/piper-rt/galion-piper-rt",
                "weights/runtime/piper/piper",
                "var/piper/piper",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
        })?;
    let model = env::var_os("GALION_PIPER_MODEL")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            [
                "weights/tts/en_US-ljspeech-medium.onnx",
                "var/voices/en_US-ljspeech-medium.onnx",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
        })?;
    let config = env::var_os("GALION_PIPER_CONFIG")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(format!("{}.json", model.display())));
    if config.is_file() {
        Some((bin, model, config))
    } else {
        None
    }
}

fn sample_rate_of(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let key = "\"sample_rate\"";
    let i = text.find(key)?;
    let digits: String = text[i + key.len()..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn run_child(
    mut cmd: Command,
    stdin_bytes: &[u8],
    timeout: Duration,
) -> Result<(i32, Vec<u8>, String), String> {
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_bytes);
    }
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stdout {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stderr {
            let _ = pipe.read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_thread.join();
                let _ = err_thread.join();
                return Err("timed out".into());
            }
            Ok(None) => thread::sleep(Duration::from_millis(30)),
            Err(err) => return Err(err.to_string()),
        }
    };
    let pcm = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();
    Ok((status.code().unwrap_or(1), pcm, err))
}

fn read_question(path: &Path) -> Result<Vec<i16>, String> {
    let (rate, pcm) = read_wav(path)?;
    let mut pcm = to_16k(&pcm, rate);
    lift_quiet(&mut pcm);
    if pcm.iter().all(|sample| *sample == 0) {
        return Err(format!("{} is silence", path.display()));
    }
    Ok(pcm)
}

/// Same peak lift as `horbm/crates/horbm-e2e/src/wav.rs` `lift_quiet`.
fn lift_quiet(samples: &mut [i16]) {
    let peak = samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    if peak == 0 || peak >= 4_000 {
        return;
    }
    let gain = 8_000.0 / f32::from(peak);
    for sample in samples.iter_mut() {
        let scaled = f32::from(*sample) * gain;
        *sample = scaled.round().clamp(-32_767.0, 32_767.0) as i16;
    }
}

fn to_16k(samples: &[i16], rate: u32) -> Vec<i16> {
    if rate == 16_000 {
        return samples.to_vec();
    }
    if rate == 0 || samples.is_empty() {
        return Vec::new();
    }
    let out_len = (samples.len() as u64).saturating_mul(16_000) / u64::from(rate);
    let mut out = Vec::with_capacity(out_len as usize);
    for i in 0..out_len {
        let src = i as f64 * f64::from(rate) / 16_000.0;
        let j = src.floor() as usize;
        let frac = src - j as f64;
        let a = f64::from(samples[j.min(samples.len() - 1)]);
        let b = f64::from(samples[(j + 1).min(samples.len() - 1)]);
        let mixed = (a + (b - a) * frac).round().clamp(-32_768.0, 32_767.0);
        out.push(mixed as i16);
    }
    out
}

fn write_wav(path: &Path, rate: u32, samples: &[i16]) -> Result<(), String> {
    if rate == 0 {
        return Err("wav rate is 0".into());
    }
    let data_len = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| "wav is too long".to_string())?;
    let riff = 36u32
        .checked_add(u32::try_from(data_len).map_err(|_| "wav is too long".to_string())?)
        .ok_or_else(|| "wav is too long".to_string())?;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&rate.saturating_mul(2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn read_wav(path: &Path) -> Result<(u32, Vec<i16>), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_wav(&bytes)
}

fn parse_wav(bytes: &[u8]) -> Result<(u32, Vec<i16>), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not RIFF/WAVE".into());
    }
    let mut index = 12usize;
    let mut rate = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut data: Option<&[u8]> = None;
    while index + 8 <= bytes.len() {
        let id = &bytes[index..index + 4];
        let size =
            u32::from_le_bytes(bytes[index + 4..index + 8].try_into().unwrap_or([0; 4])) as usize;
        let start = index + 8;
        let end = start.saturating_add(size).min(bytes.len());
        let body = &bytes[start..end];
        if id == b"fmt " && body.len() >= 16 {
            channels = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            data = Some(body);
        }
        index = start.saturating_add(size);
        if size % 2 == 1 {
            index = index.saturating_add(1);
        }
    }
    let data = data.ok_or_else(|| "wav has no data".to_string())?;
    if bits != 16 || rate == 0 || (channels != 1 && channels != 2) {
        return Err(format!(
            "want 16-bit mono or stereo, got {bits}-bit {channels} ch {rate} Hz"
        ));
    }
    let samples: Vec<i16> = data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    if channels == 1 {
        return Ok((rate, samples));
    }
    let mono = samples
        .chunks_exact(2)
        .map(|pair| ((i32::from(pair[0]) + i32::from(pair[1])) / 2) as i16)
        .collect();
    Ok((rate, mono))
}

fn http_get(addr: &SocketAddr, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(addr, Duration::from_secs(3))
        .map_err(|e| format!("connect {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        host_header(addr)
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let (head, body) = split_http(&text).ok_or_else(|| "short HTTP response".to_string())?;
    let status = head.split_whitespace().nth(1).unwrap_or("0");
    if status != "200" {
        return Err(format!("HTTP {status}"));
    }
    Ok(body.to_string())
}

fn open_session(addr: &SocketAddr) -> Result<String, String> {
    let mut stream =
        TcpStream::connect_timeout(addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let body = "{\"op\":\"open\"}";
    let req = format!(
        "POST /session HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        host_header(addr),
        body.len()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let (_, resp) = split_http(&text).ok_or_else(|| "short session response".to_string())?;
    json_string(resp, "token").ok_or_else(|| "no token".to_string())
}

fn host_header(addr: &SocketAddr) -> String {
    match addr.ip() {
        std::net::IpAddr::V4(ip) => format!("{ip}:{}", addr.port()),
        std::net::IpAddr::V6(ip) => format!("[{ip}]:{}", addr.port()),
    }
}

fn split_http(text: &str) -> Option<(&str, &str)> {
    text.split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
}

fn drive(
    addr: &SocketAddr,
    token: &str,
    pcm: &[i16],
    mode: Mode,
    wait: Duration,
) -> Result<Obs, String> {
    let mut stream =
        TcpStream::connect_timeout(addr, Duration::from_secs(5)).map_err(|e| e.to_string())?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|e| e.to_string())?;
    let key = b64(&mask_bytes(16));
    let req = format!(
        "GET /v1/samantha HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nOrigin: http://{}\r\n\r\n",
        host_header(addr),
        host_header(addr)
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    read_until_header(&mut stream, &mut buf)?;
    let header_end =
        find_header_end(&buf).ok_or_else(|| "websocket upgrade had no header".to_string())?;
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let status = head.split_whitespace().nth(1).unwrap_or("0");
    if status != "101" {
        return Err(format!("websocket upgrade HTTP {status}"));
    }
    buf.drain(..header_end);

    let session = if token.is_empty() {
        "null".to_string()
    } else {
        format!("\"{}\"", json_escape(token))
    };
    let hello = format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"hello\",\"session\":{session}}}");
    write_client(&mut stream, 1, hello.as_bytes())?;
    let welcome_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() > welcome_deadline {
            return Err("no welcome".into());
        }
        match read_ws(&mut stream, &mut buf, false)? {
            None => continue,
            Some((9, payload)) => write_client(&mut stream, 10, &payload)?,
            Some((8, _)) => return Err("socket closed before welcome".into()),
            Some((1, payload)) => {
                let text = String::from_utf8_lossy(&payload);
                let kind = json_string(&text, "type").unwrap_or_default();
                if kind == "welcome" {
                    break;
                }
                if kind == "error" {
                    return Err(format!(
                        "hello error {}",
                        json_string(&text, "error").unwrap_or_else(|| "error".into())
                    ));
                }
            }
            Some(_) => {}
        }
    }

    let mut seq = 0u16;
    let mut eos = None;
    for frame in voiced_frames(pcm) {
        write_client(&mut stream, 2, &encode_pcm(0, 1, seq, &frame))?;
        seq = seq.wrapping_add(1);
        if frame.iter().any(|sample| *sample != 0) {
            eos = Some(Instant::now());
        }
    }
    let eos = eos.ok_or_else(|| "question PCM was silence".to_string())?;
    let silent = vec![0i16; FRAME];
    for _ in 0..HANG_FRAMES {
        write_client(&mut stream, 2, &encode_pcm(0, 1, seq, &silent))?;
        seq = seq.wrapping_add(1);
    }

    let mut obs = Obs {
        replies: Vec::new(),
        error: String::new(),
        speaker_bytes: 0,
        ttfa_ms: None,
        ttfa_anchor: "unmeasured".into(),
        from_eos_ms: None,
        barge_stop_ms: None,
        barge_before: 0,
        barge_after_idle: 0,
        barge_note: String::new(),
    };
    let deadline = Instant::now() + wait;
    let mut t_vad: Option<Instant> = None;
    let mut saw_pcm = false;
    let mut stop_sent: Option<Instant> = None;
    let mut saw_idle = false;
    while Instant::now() < deadline && !saw_idle {
        let frame = match read_ws(&mut stream, &mut buf, false) {
            Ok(frame) => frame,
            Err(err) => {
                obs.error = err;
                break;
            }
        };
        let Some((opcode, payload)) = frame else {
            continue;
        };
        if opcode == 9 {
            write_client(&mut stream, 10, &payload)?;
            continue;
        }
        if opcode == 8 {
            break;
        }
        if opcode == 1 {
            let text = String::from_utf8_lossy(&payload).into_owned();
            match json_string(&text, "type").as_deref() {
                Some("turn") => {
                    if let Some(reply) = json_string(&text, "reply") {
                        obs.replies.push(reply);
                    }
                }
                Some("vad") => {
                    if json_string(&text, "event").as_deref() == Some("end") && t_vad.is_none() {
                        t_vad = Some(Instant::now());
                    }
                }
                Some("error") => {
                    obs.error = json_string(&text, "error").unwrap_or_else(|| "error".into());
                }
                Some("dot") => {
                    if json_string(&text, "state").as_deref() == Some("idle")
                        && (saw_pcm || !obs.replies.is_empty() || stop_sent.is_some() || !obs.error.is_empty())
                    {
                        if let Some(sent) = stop_sent {
                            obs.barge_stop_ms = Some(elapsed_ms(sent, Instant::now()));
                        }
                        saw_idle = true;
                    }
                }
                _ => {}
            }
            continue;
        }
        if opcode != 2 {
            continue;
        }
        let Some((dir, _rate, samples)) = decode_pcm(&payload) else {
            continue;
        };
        if dir != 1 || samples.is_empty() {
            continue;
        }
        let now = Instant::now();
        obs.speaker_bytes = obs.speaker_bytes.saturating_add((samples.len() * 2) as u64);
        if !saw_pcm {
            saw_pcm = true;
            obs.from_eos_ms = Some(elapsed_ms(eos, now));
            if let Some(vad) = t_vad {
                if vad <= now {
                    obs.ttfa_ms = Some(elapsed_ms(vad, now));
                    obs.ttfa_anchor = "vad-end".into();
                }
            }
            if obs.ttfa_ms.is_none() {
                obs.ttfa_ms = obs.from_eos_ms;
                obs.ttfa_anchor = "eos".into();
            }
            if mode == Mode::Barge {
                if unread_has_idle(&buf) {
                    obs.barge_note = "the reply finished before stop was sent".into();
                    obs.barge_before = 1;
                } else {
                    let stop = format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"stop\"}}");
                    write_client(&mut stream, 1, stop.as_bytes())?;
                    stop_sent = Some(Instant::now());
                    obs.barge_before = 1;
                }
            }
        } else if saw_idle {
            obs.barge_after_idle = obs.barge_after_idle.saturating_add(1);
        }
    }
    if mode == Mode::Barge && stop_sent.is_some() && !saw_idle {
        obs.barge_stop_ms = None;
        obs.barge_note = "stop was sent and no idle dot came back".into();
    }
    if mode == Mode::Barge && !saw_pcm && obs.barge_note.is_empty() {
        obs.barge_note = "no speaker frame arrived to stop".into();
    }
    if saw_idle && mode == Mode::Barge {
        let drain_until = Instant::now() + Duration::from_millis(250);
        while Instant::now() < drain_until {
            match read_ws(&mut stream, &mut buf, false) {
                Ok(Some((2, payload))) => {
                    if let Some((1, _, samples)) = decode_pcm(&payload) {
                        if !samples.is_empty() {
                            obs.barge_after_idle = obs.barge_after_idle.saturating_add(1);
                        }
                    }
                }
                Ok(Some((9, payload))) => {
                    let _ = write_client(&mut stream, 10, &payload);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    if !saw_pcm && obs.error.is_empty() && Instant::now() >= deadline {
        obs.error = "no speaker audio before the deadline".into();
    }
    Ok(obs)
}

fn elapsed_ms(from: Instant, to: Instant) -> u128 {
    to.saturating_duration_since(from).as_millis()
}

fn voiced_frames(pcm: &[i16]) -> Vec<Vec<i16>> {
    let mut frames = Vec::new();
    let mut off = 0;
    while off < pcm.len() {
        let end = (off + FRAME).min(pcm.len());
        let mut frame = pcm[off..end].to_vec();
        if frame.len() < FRAME {
            frame.resize(FRAME, 0);
        }
        frames.push(frame);
        off = end;
    }
    frames
}

fn unread_has_idle(buf: &[u8]) -> bool {
    buf.windows(13).any(|window| window == b"\"state\":\"idle\"")
}

fn encode_pcm(dir: u8, rate: u8, seq: u16, pcm: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + pcm.len() * 2);
    out.extend_from_slice(&[b'G', b'S', 1, dir, rate, 0]);
    out.extend_from_slice(&seq.to_le_bytes());
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn decode_pcm(frame: &[u8]) -> Option<(u8, u8, Vec<i16>)> {
    if frame.len() < 8 || frame[0] != b'G' || frame[1] != b'S' || frame[2] != 1 || frame.len() % 2 != 0
    {
        return None;
    }
    let samples = frame[8..]
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some((frame[3], frame[4], samples))
}

fn write_client(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> Result<(), String> {
    let mask = mask_bytes(4);
    let mut mask4 = [0u8; 4];
    mask4.copy_from_slice(&mask);
    stream
        .write_all(&encode_client_frame(opcode, payload, mask4))
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn encode_client_frame(opcode: u8, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + payload.len());
    out.push(0x80 | opcode);
    push_len(&mut out, payload.len(), true);
    out.extend_from_slice(&mask);
    for (i, byte) in payload.iter().enumerate() {
        out.push(byte ^ mask[i % 4]);
    }
    out
}

#[cfg(test)]
fn encode_server_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + payload.len());
    out.push(0x80 | opcode);
    push_len(&mut out, payload.len(), false);
    out.extend_from_slice(payload);
    out
}

fn push_len(out: &mut Vec<u8>, n: usize, mask: bool) {
    let bit = if mask { 0x80 } else { 0 };
    if n < 126 {
        out.push(bit | n as u8);
    } else if n <= 65_535 {
        out.push(bit | 126);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(bit | 127);
        out.extend_from_slice(&(n as u64).to_be_bytes());
    }
}

fn read_ws(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    masked: bool,
) -> Result<Option<(u8, Vec<u8>)>, String> {
    loop {
        if let Some((opcode, payload, used)) = take_frame(buf, masked)? {
            buf.drain(..used);
            return Ok(Some((opcode, payload)));
        }
        let mut tmp = [0u8; 4096];
        match stream.read(&mut tmp) {
            Ok(0) => return Err("socket closed".into()),
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::TimedOut
                    || err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                return Ok(None);
            }
            Err(err) => return Err(err.to_string()),
        }
    }
}

fn take_frame(buf: &[u8], expect_mask: bool) -> Result<Option<(u8, Vec<u8>, usize)>, String> {
    if buf.len() < 2 {
        return Ok(None);
    }
    if buf[0] & 0x80 == 0 {
        return Err("fragmented websocket frame".into());
    }
    let opcode = buf[0] & 0x0f;
    let masked = buf[1] & 0x80 != 0;
    if masked != expect_mask {
        return Err("unexpected websocket mask bit".into());
    }
    let mut len = (buf[1] & 0x7f) as usize;
    let mut pos = 2usize;
    if len == 126 {
        if buf.len() < 4 {
            return Ok(None);
        }
        len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        pos = 4;
    } else if len == 127 {
        if buf.len() < 10 {
            return Ok(None);
        }
        let n = u64::from_be_bytes(buf[2..10].try_into().unwrap_or([0; 8]));
        if n > 1_000_000 {
            return Err("websocket frame is too large".into());
        }
        len = n as usize;
        pos = 10;
    }
    if len > 1_000_000 {
        return Err("websocket frame is too large".into());
    }
    let mask_len = if masked { 4 } else { 0 };
    let total = pos + mask_len + len;
    if buf.len() < total {
        return Ok(None);
    }
    let mut payload = buf[pos + mask_len..total].to_vec();
    if masked {
        let mask = &buf[pos..pos + 4];
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }
    Ok(Some((opcode, payload, total)))
}

fn read_until_header(stream: &mut TcpStream, buf: &mut Vec<u8>) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while find_header_end(buf).is_none() {
        if Instant::now() > deadline {
            return Err("websocket upgrade timed out".into());
        }
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => return Err("socket closed during upgrade".into()),
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(err) => return Err(err.to_string()),
        }
        if buf.len() > 16_384 {
            return Err("websocket header is too large".into());
        }
    }
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn mask_bytes(n: usize) -> Vec<u8> {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let mut x = tick as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        x ^= x << 7;
        x ^= x >> 9;
        x ^= x << 8;
        out.push((x & 0xff) as u8);
    }
    if out.iter().all(|b| *b == 0) {
        out[0] = 1;
    }
    out
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i < data.len() {
        let a = data[i] as u32;
        let b = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let n = (a << 16) | (b << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if i + 1 < data.len() {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        out.push('=');
    }
    out
}

fn json_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn json_string(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let mut rest = text;
    while let Some(i) = rest.find(&pattern) {
        let after = rest[i + pattern.len()..].trim_start();
        if let Some(after) = after.strip_prefix(':') {
            let after = after.trim_start();
            if let Some(raw) = after.strip_prefix('"') {
                return finish_string(raw);
            }
        }
        rest = &rest[i + pattern.len()..];
    }
    None
}

fn finish_string(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return None;
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                Some(other) => out.push(other),
                None => return None,
            }
            continue;
        }
        if c == '"' {
            return Some(out);
        }
        out.push(c);
    }
    None
}

fn json_bool(text: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{key}\"");
    let i = text.find(&pattern)?;
    let after = text[i + pattern.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_number(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let i = text.find(&pattern)?;
    let after = text[i + pattern.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .take(240)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const PERSONA: &str = include_str!("../docs/galion-voice-v2/personas/SAMANTHA.md");
    const ROUTE: &str = include_str!("../horbm/crates/horbm/src/route.rs");
    const CLOCK: &str = include_str!("../horbm/crates/horbm-voice/src/clock.rs");

    #[test]
    fn phrases_match_the_tree() {
        assert!(PERSONA.contains(NOT_HUMAN));
        assert!(ROUTE.contains("pub const UNKNOWN_REPLY: &str = \"I don't know.\""));
        assert!(CLOCK.contains("pub const BARGE_IN_MAX_MS: u32 = 200;"));
        assert_eq!(MISS, "I don't know.");
    }

    #[test]
    fn loopback_only() {
        assert!(parse_bind("127.0.0.1:7881").is_ok());
        assert!(parse_bind("[::1]:7881").is_ok());
        assert!(parse_bind("0.0.0.0:7881").is_err());
        assert!(parse_bind("8.8.8.8:7881").is_err());
        assert!(parse_bind("192.168.1.20:7881").is_err());
        assert!(!bind_is_loopback("0.0.0.0:7881"));
        assert!(bind_is_loopback("127.0.0.1:7881"));
        assert!(bind_is_loopback("[::1]:7881"));
    }

    #[test]
    fn a_missing_clock_is_not_a_pass() {
        let mut scores = sample_pass();
        assert!(verdict(&scores));
        scores.ttfa_ms = None;
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.barge_stop_ms = None;
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.barge_stop_ms = Some(201);
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.barge_after_idle = 1;
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.mouth = "fake".into();
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.speaker_bytes = 0;
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.miss_reply = "I don't know".into();
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.not_human_reply = "I'm a voice program, not a person.".into();
        assert!(!verdict(&scores));
        scores = sample_pass();
        scores.not_human_reply = format!("I'm Samantha. {NOT_HUMAN}");
        assert!(verdict(&scores));
        let text = render(&sample_pass(), true);
        assert!(text.ends_with("result=PASS\n"));
        assert!(!render(&empty_scores(), false).contains("ttfa_ms=0\n"));
        assert!(render(&empty_scores(), false).contains("ttfa_ms=unmeasured\n"));
    }

    #[test]
    fn wav_roundtrip_and_resample() {
        let dir = env::temp_dir().join(format!("galion-live-wav-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        let pcm = vec![0i16, 1000, -1000, 32000];
        write_wav(&path, 8_000, &pcm).unwrap();
        let (rate, back) = read_wav(&path).unwrap();
        assert_eq!(rate, 8_000);
        assert_eq!(back, pcm);
        let up = to_16k(&pcm, 8_000);
        assert_eq!(up.len(), 8);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn flite_wav_is_audible_and_drives_the_fixture_when_flite_is_installed() {
        let tmp = match TempWav::new() {
            Ok(tmp) => tmp,
            Err(err) => panic!("{err}"),
        };
        let pcm = match synth(&tmp.dir, "human", HUMAN_Q) {
            Ok(kind) => {
                assert!(kind == "flite" || kind == "piper", "{kind}");
                read_question(&tmp.dir.join("human.wav")).expect("question wav")
            }
            Err(err) => {
                assert!(
                    err.contains("did not run") || err.contains("No such file"),
                    "synth failed with flite or piper present: {err}"
                );
                return;
            }
        };
        let peak = pcm.iter().map(|sample| sample.unsigned_abs()).max().unwrap_or(0);
        assert!(peak >= 4_000, "synthesised peak {peak}");
        assert!(pcm.len() > 8_000, "synthesised samples {}", pcm.len());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().unwrap();
                sock.set_nodelay(true).unwrap();
                sock.set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                serve_fixture(&mut sock);
            }
        });
        let full = drive(&addr, "", &pcm, Mode::Full, Duration::from_secs(8)).expect("full");
        assert_eq!(full.reply(), NOT_HUMAN);
        assert!(full.ttfa_ms.is_some(), "{}", full.ttfa_anchor);
        assert!(full.speaker_bytes > 0);
        let barge = drive(&addr, "", &pcm, Mode::Barge, Duration::from_secs(8)).expect("barge");
        let stop = barge.barge_stop_ms.expect(&barge.barge_note);
        assert!(stop <= BARGE_LOCK_MS, "barge_stop_ms {stop}");
        assert_eq!(barge.barge_after_idle, 0);
        server.join().unwrap();
    }

    #[test]
    fn fixture_server_measures_ttfa_reply_and_barge() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().unwrap();
                sock.set_nodelay(true).unwrap();
                sock.set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                serve_fixture(&mut sock);
            }
        });
        let pcm = vec![8_000i16; FRAME * 8];
        let full = drive(&addr, "", &pcm, Mode::Full, Duration::from_secs(4)).expect("full");
        assert_eq!(full.reply(), NOT_HUMAN);
        assert!(full.ttfa_ms.is_some(), "ttfa {}", full.ttfa_anchor);
        assert!(full.speaker_bytes > 0);
        assert_eq!(full.ttfa_anchor, "vad-end");
        let barge = drive(&addr, "", &pcm, Mode::Barge, Duration::from_secs(4)).expect("barge");
        assert!(barge.barge_before >= 1, "{}", barge.barge_note);
        assert_eq!(barge.barge_after_idle, 0, "{}", barge.barge_note);
        let stop = barge.barge_stop_ms.expect(&barge.barge_note);
        assert!(stop <= BARGE_LOCK_MS, "barge_stop_ms {stop}");
        server.join().unwrap();
    }

    fn sample_pass() -> Scores {
        let mut scores = empty_scores();
        scores.health = true;
        scores.audio_in = true;
        scores.mouth = "piper".into();
        scores.speaker_bytes = 1920;
        scores.ttfa_ms = Some(40);
        scores.ttfa_anchor = "vad-end".into();
        scores.from_eos_ms = Some(700);
        scores.barge_stop_ms = Some(8);
        scores.barge_before = 1;
        scores.barge_after_idle = 0;
        scores.not_human_reply = NOT_HUMAN.into();
        scores.miss_reply = MISS.into();
        scores
    }

    fn serve_fixture(sock: &mut TcpStream) {
        let mut buf = Vec::new();
        let _ = read_http(sock, &mut buf);
        let _ = sock.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        let welcome = format!(
            "{{\"schema\":\"{SCHEMA}\",\"type\":\"welcome\",\"frame_ms\":80,\"sample_rate\":24000,\"dot\":\"idle\"}}"
        );
        expect_text(sock, &mut buf);
        let _ = sock.write_all(&encode_server_frame(1, welcome.as_bytes()));
        let mut voiced = false;
        let mut zeros = 0u32;
        loop {
            match read_ws(sock, &mut buf, true) {
                Ok(Some((2, payload))) => {
                    let silent = decode_pcm(&payload)
                        .map(|(_, _, samples)| samples.iter().all(|s| *s == 0))
                        .unwrap_or(true);
                    if !silent {
                        voiced = true;
                        zeros = 0;
                    } else if voiced {
                        zeros += 1;
                    }
                    if voiced && zeros >= 32 {
                        break;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => {}
                Err(_) => return,
            }
        }
        let vad = format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"vad\",\"event\":\"end\",\"t_ms\":1}}");
        let turn = format!(
            "{{\"schema\":\"{SCHEMA}\",\"type\":\"turn\",\"reply\":\"{}\"}}",
            json_escape(NOT_HUMAN)
        );
        let pcm = encode_pcm(1, 0, 1, &[1000i16; 320]);
        let _ = sock.write_all(&encode_server_frame(1, vad.as_bytes()));
        let _ = sock.write_all(&encode_server_frame(1, turn.as_bytes()));
        let _ = sock.write_all(&encode_server_frame(2, &pcm));
        let _ = sock.flush();
        let wait = Instant::now() + Duration::from_millis(800);
        let mut stopped = false;
        while Instant::now() < wait {
            match read_ws(sock, &mut buf, true) {
                Ok(Some((1, payload))) => {
                    let text = String::from_utf8_lossy(&payload);
                    if json_string(&text, "type").as_deref() == Some("stop") {
                        stopped = true;
                        break;
                    }
                }
                Ok(Some((9, payload))) => {
                    let _ = sock.write_all(&encode_server_frame(10, &payload));
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        if !stopped {
            let more = encode_pcm(1, 0, 2, &[1000i16; 320]);
            let _ = sock.write_all(&encode_server_frame(2, &more));
        }
        let idle = format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"dot\",\"state\":\"idle\",\"level\":0}}");
        let _ = sock.write_all(&encode_server_frame(1, idle.as_bytes()));
        let _ = sock.flush();
    }

    fn read_http(sock: &mut TcpStream, buf: &mut Vec<u8>) {
        let _ = read_until_header(sock, buf);
        if let Some(end) = find_header_end(buf) {
            buf.drain(..end);
        }
    }

    fn expect_text(sock: &mut TcpStream, buf: &mut Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match read_ws(sock, buf, true) {
                Ok(Some((1, _))) => return,
                Ok(Some((9, payload))) => {
                    let _ = sock.write_all(&encode_server_frame(10, &payload));
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }
}
