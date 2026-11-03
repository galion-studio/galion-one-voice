// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! S4+S5+S6 · **phone turn**: 16 kHz PCM/WAV in → transcript → reply text → 24 kHz
//! spoken WAV out (feature `cascade`). ENGLISH ONLY, CPU, fallback lane (never the
//! 80 ms primary path).
//!
//! ```text
//!  pcm16k ──S4 PhoneTurnAsr──▶ transcript ──S5 Hermes or CannedLlm──▶ reply_text ──S6 SpeechStack──▶ wav_bytes (24 kHz)
//! ```
//!
//! # S4 ASR — letter A, Vosk, out of process
//! The owner chose letter A (2026-11-03): Vosk, CPU, separate process.
//! [`PhoneTurnAsr`] spawns `galion-asr-vosk`, which dlopens libvosk. This crate
//! does not link it. `LOCKS.toml` is not edited. See `docs/asr/VOSK.md`.
//!
//! | `GALION_ASR`     | behaviour |
//! |------------------|-----------|
//! | unset / `stub`   | **library default.** Stub fixtures decode. Real speech → [`PhoneError::AsrUnratified`]. Nothing is invented. |
//! | `vosk`           | Real words from `vosk-model-small-en-us-0.15`. Missing model or worker → [`PhoneError::AsrUnavailable`]. A blank decode → [`PhoneError::Empty`]. |
//! | `off`            | Always [`PhoneError::AsrUnratified`]. |
//! | `interim`        | **Ignored.** Treated as `stub`. The length-bucket sentences are [`AsrMode::Interim`], which tests construct directly. The live screen never selects them. |
//!
//! Every [`PhoneTurn`] carries `asr` so a UI can tell a Vosk transcript from a fixture.
//!
//! # S5 LLM
//! Phone turns default to [`CannedLlm`] when `GALION_LLM` is unset or `canned`
//! (explicit test opt-in). `GALION_LLM=echo` selects the old `MockLlm` echo.
//! `GALION_LLM=hermes` selects [`crate::HermesLlm`]: a loopback OpenAI chat
//! completion (`GALION_LLM_URL`, default `http://127.0.0.1:8642/v1/chat/completions`).
//! Any other value is [`PhoneError::UnknownLlm`]. It is not canned.
//! A timeout or a dead gateway speaks `model can't be reached` and does not
//! fall back to the rule table. Emotion is fixed `Neutral`: inferring affect
//! from the caller's voice is the LOCKS `[consent] sense` bit (default deny).
//!
//! # S6 TTS
//! [`SpeechStack::detect`] (A21 Piper VITS `en_US-ljspeech-medium` if the sha256-verified
//! pack + Piper runtime are on disk, else libflite). The A21 path keeps **one** Piper
//! process for the life of the lane (stdin JSON, stdout raw PCM). [`PhoneLane::new`]
//! pre-warms it with a discarded "Hello." so the first caller turn is warm. No backend
//! → [`PhoneError::NoSpeech`]; tones are never returned as a reply (FRXDEL Q2).
//!
//! # Threads
//! [`PhoneLane`] is `Send`: build it once on a worker thread and call [`PhoneLane::turn`]
//! per utterance (the speech stack and the Piper process are reused). [`phone_turn`] /
//! [`phone_turn_wav`] are one-shot conveniences that read env each call.

use crate::canned::CannedLlm;
use crate::en_lane::{spoken_text, to_f32, to_pcm16, wav_bytes, LANE_RATE_HZ};
use crate::hermes::HermesLlm;
use crate::{CascadeError, Emotion, Lang, Llm, MockLlm, Transcript};
use galion_ml_voice::speech::{resample, SpeechStack};

pub use galion_ml_voice::speech::FrameProsody;
use galion_ml_voice::stub::SAMPLES_PER_CHAR;
use galion_ml_voice::VoiceError;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;

/// Phone-path input rate (16 kHz mono PCM16).
pub const PHONE_IN_RATE_HZ: u32 = crate::SAMPLE_RATE_HZ;
/// Output WAV rate (24 kHz mono PCM16, Mimi-class lane bus).
pub const PHONE_OUT_RATE_HZ: u32 = LANE_RATE_HZ;
/// Pointer embedded in every "not ratified" error.
pub const RATIFY_DOC: &str = "docs/policy/RATIFY-EN-ASR-PHONE-PATH.md";

/// S4 ASR mode (see module docs; `GALION_ASR`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrMode { Stub, Interim, Vosk, Off }

impl AsrMode {
    pub fn from_env() -> Self { Self::from_choice(std::env::var("GALION_ASR").ok().as_deref()) }
    /// Live selection. `interim` is not a production mode: it maps to [`AsrMode::Stub`].
    pub fn from_choice(v: Option<&str>) -> Self {
        match v.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("vosk") => AsrMode::Vosk,
            Some("off") => AsrMode::Off,
            // Length buckets are `AsrMode::Interim`, constructed by tests. The env value must not invent a transcript.
            Some("interim") => AsrMode::Stub,
            _ => AsrMode::Stub,
        }
    }
}

/// S5 LLM mode (`GALION_LLM`). Unset and `canned` are the rule table.
/// `hermes` is the loopback gateway. Anything else is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmMode { Canned, Echo, Hermes }

impl LlmMode {
    pub fn from_env() -> Result<Self, PhoneError> {
        Self::from_choice(std::env::var("GALION_LLM").ok().as_deref())
    }
    /// `None`, `""`, and `canned` → [`LlmMode::Canned`]. `echo` / `mock` → echo.
    /// `hermes` → [`LlmMode::Hermes`]. Every other value is [`PhoneError::UnknownLlm`].
    pub fn from_choice(v: Option<&str>) -> Result<Self, PhoneError> {
        let raw = v.unwrap_or("").trim();
        match raw.to_ascii_lowercase().as_str() {
            "" | "canned" => Ok(LlmMode::Canned),
            "echo" | "mock" => Ok(LlmMode::Echo),
            "hermes" => Ok(LlmMode::Hermes),
            _ => Err(PhoneError::UnknownLlm(raw.to_string())),
        }
    }
}

/// Phone-turn configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhoneConfig { pub asr: AsrMode, pub llm: LlmMode }

impl PhoneConfig {
    /// `GALION_ASR` + `GALION_LLM`. Unknown `GALION_LLM` is an error.
    pub fn from_env() -> Result<Self, PhoneError> {
        Ok(Self { asr: AsrMode::from_env(), llm: LlmMode::from_env()? })
    }
}
impl Default for PhoneConfig { fn default() -> Self { Self { asr: AsrMode::Stub, llm: LlmMode::Canned } } }

/// Phone-turn failure.
#[derive(Debug, Clone, PartialEq)]
pub enum PhoneError {
    /// No audio, or only silence.
    Empty,
    /// WAV bytes could not be parsed (only PCM16 RIFF is accepted).
    BadWav(String),
    /// Real speech arrived and this lane is not running Vosk (see [`RATIFY_DOC`]).
    AsrUnratified(&'static str),
    /// `GALION_ASR=vosk` but the model, library, or worker could not produce words.
    /// The message is the failure. The transcript was not invented.
    AsrUnavailable(String),
    /// No speech backend on this machine (Piper/flite missing); never falls back to tones.
    NoSpeech(String),
    /// A newer turn superseded this one between clauses. The Piper process is still running.
    Superseded,
    /// Error from the cascade (LLM / route).
    Cascade(CascadeError),
    /// `GALION_LLM` is not `canned`, `echo`/`mock`, or `hermes`. Not a canned fallback.
    UnknownLlm(String),
}

impl std::fmt::Display for PhoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhoneError::Empty => write!(f, "empty or silent audio"),
            PhoneError::BadWav(m) => write!(f, "bad WAV: {m}"),
            PhoneError::AsrUnratified(m) => write!(f, "{m}"),
            PhoneError::AsrUnavailable(m) => write!(f, "{m}"),
            PhoneError::NoSpeech(m) => write!(f, "no speech backend: {m}"),
            PhoneError::Superseded => write!(f, "turn superseded before speech finished"),
            PhoneError::Cascade(e) => write!(f, "cascade: {e:?}"),
            PhoneError::UnknownLlm(m) => write!(f, "unknown GALION_LLM '{m}'; use canned, echo, or hermes"),
        }
    }
}
impl std::error::Error for PhoneError {}
impl From<CascadeError> for PhoneError {
    fn from(e: CascadeError) -> Self { if e == CascadeError::Empty { PhoneError::Empty } else { PhoneError::Cascade(e) } }
}

/// Which ASR produced the transcript (label it in the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrSource {
    /// Exact decode of a `galion_ml_voice::stub` fixture.
    StubFixture,
    /// INTERIM PLACEHOLDER: canned text, the words were NOT recognised.
    InterimPlaceholder,
    /// Virtual-user text injection. The harness supplied the words. Not speech recognition.
    TextInject,
    /// Words from the Vosk worker. Not a length bucket.
    Vosk,
}
impl AsrSource {
    pub fn id(&self) -> &'static str {
        match self {
            AsrSource::StubFixture => "stub-fixture",
            AsrSource::InterimPlaceholder => "interim-placeholder",
            AsrSource::TextInject => "text-inject",
            AsrSource::Vosk => "vosk",
        }
    }
    /// Stub fixtures are an exact codec decode. Vosk is speech recognition.
    /// Interim and text-inject are not.
    pub fn is_recognised(&self) -> bool { matches!(self, AsrSource::StubFixture | AsrSource::Vosk) }
}

/// Per-stage wall time (ms).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StageMs {
    pub asr: u32,
    pub llm: u32,
    pub tts: u32,
    /// Speak entry → first clause PCM. The rest of `tts` is the remainder.
    pub tts_first_pcm: u32,
}

/// One finished phone turn.
#[derive(Debug, Clone)]
pub struct PhoneTurn {
    /// What the caller said (or the interim placeholder — check `asr`).
    pub transcript: String,
    /// Reply as spoken (no `[Emotion]` tag).
    pub reply_text: String,
    /// Mono PCM16 RIFF/WAVE at [`PHONE_OUT_RATE_HZ`] (24 kHz), real spoken words.
    pub wav_bytes: Vec<u8>,
    /// Total wall time of the turn (ms).
    pub ms: u32,
    pub stage_ms: StageMs,
    pub asr: AsrSource,
    /// `canned-llm …` | `mock-llm …`.
    pub llm: &'static str,
    /// `a21-weights` | `system-flite`.
    pub speech_backend: &'static str,
    /// Human label of the speech backend.
    pub speech_label: String,
}

/// S4 ASR for the phone path. Input is 16 kHz mono PCM16.
/// The Vosk worker, when `mode` is [`AsrMode::Vosk`], stays up for the life of the lane.
pub struct PhoneTurnAsr {
    pub mode: AsrMode,
    worker: Option<galion_asr_vosk::VoskClient>,
}

impl std::fmt::Debug for PhoneTurnAsr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneTurnAsr").field("mode", &self.mode).finish()
    }
}

/// RMS (0..1) below which the input counts as silence.
pub const SILENCE_RMS: f32 = 0.003;

/// Interim placeholders by utterance length (short → long). NOT recognition.
pub const INTERIM_TRANSCRIPTS: [&str; 3] = ["hello", "hello how are you", "can you hear me"];

impl PhoneTurnAsr {
    pub fn new(mode: AsrMode) -> Self { Self { mode, worker: None } }

    pub fn transcribe(&mut self, pcm16k: &[i16]) -> Result<(String, AsrSource), PhoneError> {
        if pcm16k.is_empty() { return Err(PhoneError::Empty); }
        if self.mode == AsrMode::Off {
            return Err(PhoneError::AsrUnratified("EN phone ASR disabled (GALION_ASR=off). Typed text still works. No transcript was invented."));
        }
        if let Some(t) = decode_stub_fixture(pcm16k) { return Ok((t, AsrSource::StubFixture)); }
        let rms = (pcm16k.iter().map(|&s| (s as f32 / 32768.0).powi(2)).sum::<f32>() / pcm16k.len() as f32).sqrt();
        if rms < SILENCE_RMS { return Err(PhoneError::Empty); }
        match self.mode {
            AsrMode::Interim => {
                // Test-only length buckets. `GALION_ASR=interim` does not select this arm.
                let secs = pcm16k.len() as f32 / PHONE_IN_RATE_HZ as f32;
                let i = if secs < 1.0 { 0 } else if secs < 2.5 { 1 } else { 2 };
                Ok((INTERIM_TRANSCRIPTS[i].to_string(), AsrSource::InterimPlaceholder))
            }
            AsrMode::Vosk => self.vosk_transcribe(pcm16k),
            AsrMode::Stub | AsrMode::Off => Err(PhoneError::AsrUnratified(
                "EN phone ASR is not running on this turn (GALION_ASR=stub). Real speech needs GALION_ASR=vosk and the small-en model (docs/asr/VOSK.md). No transcript was invented.",
            )),
        }
    }

    fn vosk_transcribe(&mut self, pcm16k: &[i16]) -> Result<(String, AsrSource), PhoneError> {
        if self.worker.is_none() {
            match galion_asr_vosk::VoskClient::open() {
                Ok(w) => self.worker = Some(w),
                Err(e) => return Err(PhoneError::AsrUnavailable(e)),
            }
        }
        let grammar = vosk_grammar();
        let result = self.worker.as_mut().unwrap().recognize(pcm16k, grammar.as_deref());
        match result {
            Ok(r) => {
                let text = galion_asr_vosk::clean_transcript(&r.text);
                if text.is_empty() { Err(PhoneError::Empty) } else { Ok((text, AsrSource::Vosk)) }
            }
            Err(e) => {
                self.worker = None;
                Err(PhoneError::AsrUnavailable(e))
            }
        }
    }
}

/// `GALION_VOSK_GRAMMAR=intents` forwards the canned phrase list. Anything else is free decoding.
/// If the model cannot build that recognizer, the worker errors instead of guessing.
fn vosk_grammar() -> Option<Vec<String>> {
    let Ok(v) = std::env::var("GALION_VOSK_GRAMMAR") else { return None };
    if v.trim().eq_ignore_ascii_case("intents") {
        let mut g: Vec<String> = crate::canned::intent_grammar_phrases().into_iter().map(str::to_string).collect();
        g.push("[unk]".into());
        Some(g)
    } else {
        None
    }
}

/// PCM16 of a `galion_ml_voice` stub fixture. Tests use this so a speech-off
/// turn reaches TTS without the length-bucket placeholder.
pub fn stub_fixture_pcm16(text: &str) -> Vec<i16> {
    to_pcm16(&galion_ml_voice::stub::fixture_audio(text).samples)
}

/// Decode a `galion_ml_voice::stub::fixture_audio` signal (as PCM16) if it is one:
/// constant 240-sample blocks of printable ASCII. Otherwise `None`.
pub fn decode_stub_fixture(pcm: &[i16]) -> Option<String> {
    if pcm.is_empty() || pcm.len() % SAMPLES_PER_CHAR != 0 { return None; }
    let mut out = String::with_capacity(pcm.len() / SAMPLES_PER_CHAR);
    for c in pcm.chunks(SAMPLES_PER_CHAR) {
        if c.iter().any(|&s| s != c[0]) { return None; }
        let b = (to_f32(&c[..1])[0] * 255.0).round();
        if !(32.0..=126.0).contains(&b) { return None; }
        out.push(b as u8 as char);
    }
    let t = out.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

/// Parse PCM16 RIFF/WAVE (any rate, mono or multi-channel) → 16 kHz mono PCM16.
/// Fixture signals are returned untouched (no resample) so `StubAsr` still decodes them.
pub fn wav_to_pcm16k(b: &[u8]) -> Result<Vec<i16>, PhoneError> {
    let bad = |m: &str| PhoneError::BadWav(m.to_string());
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" { return Err(bad("not RIFF/WAVE")); }
    let (mut fmt, mut data) = (None, None);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let body = &b[i + 8..(i + 8 + n).min(b.len())];
        if id == b"fmt " { fmt = Some(body); } else if id == b"data" { data = Some(body); }
        i += 8 + n + (n & 1);
    }
    let fmt = fmt.ok_or_else(|| bad("no fmt chunk"))?;
    let data = data.ok_or_else(|| bad("no data chunk"))?;
    if fmt.len() < 16 { return Err(bad("short fmt chunk")); }
    let u16le = |o: usize| u16::from_le_bytes([fmt[o], fmt[o + 1]]);
    let (tag, ch, bits) = (u16le(0), u16le(2).max(1) as usize, u16le(14));
    let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    if !(tag == 1 || tag == 0xFFFE) || bits != 16 { return Err(bad("only PCM16 is accepted")); }
    if rate == 0 { return Err(bad("sample rate 0")); }
    let frames: Vec<i16> = data.chunks_exact(2 * ch)
        .map(|f| { let s: i32 = f.chunks_exact(2).map(|x| i16::from_le_bytes([x[0], x[1]]) as i32).sum(); (s / ch as i32) as i16 })
        .collect();
    if rate == PHONE_IN_RATE_HZ || decode_stub_fixture(&frames).is_some() { return Ok(frames); }
    Ok(to_pcm16(&resample(&to_f32(&frames), rate, PHONE_IN_RATE_HZ)))
}

/// Reusable phone lane (`Send`; keep one per worker thread).
pub struct PhoneLane {
    pub cfg: PhoneConfig,
    asr: PhoneTurnAsr,
    llm: Box<dyn Llm + Send>,
    speech: SpeechStack,
}

fn ms_since(t: Instant) -> u32 { t.elapsed().as_millis().min(u32::MAX as u128) as u32 }

impl PhoneLane {
    /// Env-configured lane (`GALION_ASR`, `GALION_LLM`, `GALION_SPEECH`, …).
    pub fn from_env() -> Result<Self, PhoneError> { Ok(Self::new(PhoneConfig::from_env()?)) }

    pub fn new(cfg: PhoneConfig) -> Self { Self::with_speech(cfg, SpeechStack::detect()) }

    /// Lane around an existing stack. Pre-warms Piper when that stack is A21.
    pub fn with_speech(cfg: PhoneConfig, mut speech: SpeechStack) -> Self {
        let llm: Box<dyn Llm + Send> = match cfg.llm {
            LlmMode::Canned => Box::new(CannedLlm::new()),
            LlmMode::Echo => Box::new(MockLlm),
            LlmMode::Hermes => Box::new(HermesLlm::from_env()),
        };
        speech.prewarm();
        Self { cfg, asr: PhoneTurnAsr::new(cfg.asr), llm, speech }
    }

    /// Register for the next Hermes completion (`samantha` or `jarvis`).
    /// Canned and echo ignore it.
    pub fn set_persona(&mut self, id: &str) { self.llm.set_persona(id); }

    /// True when S6 can produce spoken words on this machine.
    pub fn can_speak(&self) -> bool { self.speech.can_speak() }
    /// `(id, label)` of the speech backend that last produced PCM.
    /// Before the first speak, this is the selected backend.
    pub fn speech_backend(&self) -> (&'static str, String) { self.speech.reported() }

    /// Persona register and the latest 80 ms prosody. Numbers only. No reply text.
    pub fn set_delivery(&mut self, persona: &str, prosody: FrameProsody) {
        self.speech.set_persona(persona);
        self.speech.set_frame_prosody(prosody);
    }

    /// `(pid, spawn count)` of the warm Piper worker. Pid is `None` for flite / off.
    /// Spawn count `1` means every turn reused the process started with the lane.
    pub fn piper_worker(&self) -> (Option<u32>, u32) {
        let w = self.speech.warmth();
        (w.piper_pid, w.piper_spawns)
    }

    /// Drop queued Piper clauses when `turn_id` falls behind `current`. Does not kill Piper.
    pub fn arm_cancel(&mut self, current: Arc<AtomicU64>, turn_id: u64) {
        self.speech.arm_cancel(current, turn_id);
    }

    /// S4 → S5 only (no audio). Useful for UIs that show text before speech.
    pub fn reply_text(&mut self, pcm16k: &[i16]) -> Result<(String, AsrSource, String), PhoneError> {
        let (text, src) = self.asr.transcribe(pcm16k)?;
        let r = self.llm.reply(&Transcript { text: text.clone(), lang: Lang::En }, Emotion::Neutral)?;
        Ok((text, src, spoken_text(&r)))
    }

    /// Virtual-user text injection (S5 only, no audio).
    ///
    /// The words are supplied by the harness. The label is [`AsrSource::TextInject`]
    /// (`text-inject`). [`AsrSource::is_recognised`] is false: this is not recognition.
    pub fn inject_text(&mut self, text: &str) -> Result<(String, AsrSource, String), PhoneError> {
        let text = text.trim();
        if text.is_empty() { return Err(PhoneError::Empty); }
        let transcript = text.to_string();
        let r = self.llm.reply(&Transcript { text: transcript.clone(), lang: Lang::En }, Emotion::Neutral)?;
        Ok((transcript, AsrSource::TextInject, spoken_text(&r)))
    }

    /// One full turn: 16 kHz mono PCM16 → [`PhoneTurn`] with 24 kHz WAV bytes.
    pub fn turn(&mut self, pcm16k: &[i16]) -> Result<PhoneTurn, PhoneError> {
        let t0 = Instant::now();
        let (transcript, asr) = self.asr.transcribe(pcm16k)?;
        let asr_ms = ms_since(t0);
        let t1 = Instant::now();
        let raw = self.llm.reply(&Transcript { text: transcript.clone(), lang: Lang::En }, Emotion::Neutral)?;
        let reply_text = spoken_text(&raw);
        let llm_ms = ms_since(t1);
        self.emit(t0, asr_ms, llm_ms, transcript, asr, reply_text)
    }

    /// Speak `text` as given. No ASR and no canned reply.
    /// The persona floor already chose the words. ASR label is `text-inject`.
    pub fn speak_fixed(&mut self, text: &str) -> Result<PhoneTurn, PhoneError> {
        let mut owned = text.trim().to_string();
        if owned.is_empty() { return Err(PhoneError::Empty); }
        if owned.len() > 2000 { owned.truncate(2000); }
        let t0 = Instant::now();
        self.emit(t0, 0, 0, String::new(), AsrSource::TextInject, owned)
    }

    /// Full turn from harness-supplied text (CannedLlm or echo, then Piper/flite).
    /// ASR label is `text-inject`. Not speech recognition.
    pub fn turn_text(&mut self, text: &str) -> Result<PhoneTurn, PhoneError> {
        let t0 = Instant::now();
        let (transcript, asr, reply_text) = self.inject_text(text)?;
        let llm_ms = ms_since(t0);
        self.emit(t0, 0, llm_ms, transcript, asr, reply_text)
    }

    fn emit(
        &mut self,
        t0: Instant,
        asr_ms: u32,
        llm_ms: u32,
        transcript: String,
        asr: AsrSource,
        reply_text: String,
    ) -> Result<PhoneTurn, PhoneError> {
        if !self.speech.can_speak() { return Err(PhoneError::NoSpeech(self.speech.backend.label())); }
        let t2 = Instant::now();
        let audio = self.speech.speak(&reply_text).map_err(|e| match e {
            VoiceError::Cancelled => PhoneError::Superseded,
            other => PhoneError::NoSpeech(format!("{other:?}")),
        })?;
        let pcm = to_pcm16(&resample(&audio.samples, audio.sample_rate, PHONE_OUT_RATE_HZ));
        let wav = wav_bytes(&pcm, PHONE_OUT_RATE_HZ);
        let tts_ms = ms_since(t2);
        let tts_first_pcm = self.speech.warmth().last_first_pcm_ms.unwrap_or(tts_ms);
        Ok(PhoneTurn {
            transcript, reply_text, wav_bytes: wav, ms: ms_since(t0),
            stage_ms: StageMs { asr: asr_ms, llm: llm_ms, tts: tts_ms, tts_first_pcm }, asr,
            llm: self.llm.name(), speech_backend: self.speech.backend.id(), speech_label: self.speech.backend.label(),
        })
    }

    /// As [`turn`](Self::turn) from WAV bytes (PCM16, any rate; resampled to 16 kHz).
    pub fn turn_wav(&mut self, wav: &[u8]) -> Result<PhoneTurn, PhoneError> { self.turn(&wav_to_pcm16k(wav)?) }
}

/// One-shot phone turn from 16 kHz mono PCM16 (env-configured).
pub fn phone_turn(pcm16k: &[i16]) -> Result<PhoneTurn, PhoneError> { PhoneLane::from_env()?.turn(pcm16k) }
/// One-shot phone turn from WAV bytes (env-configured).
pub fn phone_turn_wav(wav: &[u8]) -> Result<PhoneTurn, PhoneError> { PhoneLane::from_env()?.turn_wav(wav) }

#[cfg(test)]
mod tests {
    use super::*;
    use galion_ml_voice::stub::fixture_audio;

    fn fx(t: &str) -> Vec<i16> { to_pcm16(&fixture_audio(t).samples) }
    fn voice(secs: f32) -> Vec<i16> {
        let n = (secs * 16_000.0) as usize;
        (0..n).map(|i| { let t = i as f32 / 16_000.0; (0.2 * (t * 180.0 * std::f32::consts::TAU).sin() * (t * 3.0 * std::f32::consts::TAU).sin().abs() * 32767.0) as i16 }).collect()
    }
    fn cfg(asr: AsrMode) -> PhoneConfig { PhoneConfig { asr, llm: LlmMode::Canned } }
    fn from_env_without(key: &str) -> AsrMode {
        let prev = std::env::var(key).ok();
        std::env::remove_var(key);
        let mode = AsrMode::from_env();
        match prev { Some(v) => std::env::set_var(key, v), None => std::env::remove_var(key) }
        mode
    }

    #[test] fn env_choices() {
        assert_eq!(AsrMode::from_choice(None), AsrMode::Stub);
        assert_eq!(AsrMode::from_choice(Some(" Interim ")), AsrMode::Stub);
        assert_eq!(AsrMode::from_choice(Some("vosk")), AsrMode::Vosk);
        assert_eq!(AsrMode::from_choice(Some(" VOSK ")), AsrMode::Vosk);
        assert_eq!(AsrMode::from_choice(Some("off")), AsrMode::Off);
        assert_eq!(AsrMode::from_choice(Some("whisper")), AsrMode::Stub);
        assert_ne!(AsrMode::from_choice(None), AsrMode::Interim);
        assert_ne!(AsrMode::from_choice(Some("interim")), AsrMode::Interim);
        assert_eq!(LlmMode::from_choice(None).unwrap(), LlmMode::Canned);
        assert_eq!(LlmMode::from_choice(Some("canned")).unwrap(), LlmMode::Canned);
        assert_eq!(LlmMode::from_choice(Some("echo")).unwrap(), LlmMode::Echo);
        assert_eq!(LlmMode::from_choice(Some(" Hermes ")).unwrap(), LlmMode::Hermes);
        assert!(matches!(LlmMode::from_choice(Some("gpt")), Err(PhoneError::UnknownLlm(m)) if m == "gpt"));
        assert!(matches!(LlmMode::from_choice(Some("openai")), Err(PhoneError::UnknownLlm(_))));
        assert!(matches!(LlmMode::from_choice(Some("hermes-agent")), Err(PhoneError::UnknownLlm(_))));
    }
    #[test] fn stub_decodes_fixture_in_every_mode_but_off() {
        for m in [AsrMode::Stub, AsrMode::Interim, AsrMode::Vosk] {
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&fx("how are you")).unwrap(), ("how are you".into(), AsrSource::StubFixture));
        }
        assert!(matches!(PhoneTurnAsr::new(AsrMode::Off).transcribe(&fx("hi")), Err(PhoneError::AsrUnratified(_))));
    }
    #[test] fn default_refuses_real_speech_without_inventing_words() {
        let e = PhoneTurnAsr::new(AsrMode::Stub).transcribe(&voice(1.5)).unwrap_err();
        assert!(matches!(e, PhoneError::AsrUnratified(m) if m.contains("docs/asr/VOSK.md") && m.contains("No transcript was invented")));
    }
    #[test] fn interim_env_is_not_the_production_path() {
        let mut a = PhoneTurnAsr::new(AsrMode::from_choice(Some("interim")));
        let err = a.transcribe(&voice(1.5)).unwrap_err();
        assert!(matches!(err, PhoneError::AsrUnratified(_)));
        let rendered = err.to_string();
        for phrase in INTERIM_TRANSCRIPTS { assert_ne!(rendered, phrase); }
        assert_eq!(from_env_without("GALION_ASR"), AsrMode::Stub);
    }
    #[test] fn interim_is_labelled_placeholder() {
        let mut a = PhoneTurnAsr::new(AsrMode::Interim);
        assert_eq!(a.transcribe(&voice(0.5)).unwrap(), ("hello".into(), AsrSource::InterimPlaceholder));
        assert_eq!(a.transcribe(&voice(1.5)).unwrap().0, "hello how are you");
        assert_eq!(a.transcribe(&voice(4.0)).unwrap().0, "can you hear me");
        assert!(!AsrSource::InterimPlaceholder.is_recognised());
    }
    #[test] fn silence_and_empty_are_empty() {
        for m in [AsrMode::Stub, AsrMode::Interim, AsrMode::Vosk] {
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&[]), Err(PhoneError::Empty));
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&vec![0; 16_000]), Err(PhoneError::Empty));
        }
    }
    #[test] fn vosk_missing_model_does_not_invent_a_transcript() {
        let prev = std::env::var("GALION_VOSK_MODEL").ok();
        std::env::set_var("GALION_VOSK_MODEL", "/no/such/galion-vosk-model");
        let mut a = PhoneTurnAsr::new(AsrMode::Vosk);
        let err = a.transcribe(&voice(0.4)).unwrap_err();
        match err {
            PhoneError::AsrUnavailable(m) => {
                assert!(m.contains("docs/asr/VOSK.md"), "{m}");
                assert!(m.contains("No transcript was invented"), "{m}");
                for phrase in INTERIM_TRANSCRIPTS { assert!(!m.contains(phrase), "{m}"); }
            }
            other => panic!("{other}"),
        }
        match prev { Some(v) => std::env::set_var("GALION_VOSK_MODEL", v), None => std::env::remove_var("GALION_VOSK_MODEL") }
    }
    #[test] fn wav_parse_mono_stereo_resample() {
        let v = voice(1.0);
        assert_eq!(wav_to_pcm16k(&wav_bytes(&v, 16_000)).unwrap(), v);
        assert_eq!(wav_to_pcm16k(&wav_bytes(&v, 48_000)).unwrap().len(), v.len() / 3);
        // stereo 16 kHz: L=R → identical mono
        let st: Vec<i16> = v.iter().flat_map(|&s| [s, s]).collect();
        let mut w = wav_bytes(&st, 16_000);
        w[22] = 2; w[32] = 4; // channels=2, block_align=4
        assert_eq!(wav_to_pcm16k(&w).unwrap(), v);
        // fixtures survive the WAV path at any declared rate
        assert_eq!(decode_stub_fixture(&wav_to_pcm16k(&wav_bytes(&fx("hi"), 24_000)).unwrap()).unwrap(), "hi");
        assert!(matches!(wav_to_pcm16k(b"nope"), Err(PhoneError::BadWav(_))));
    }
    #[test] fn text_inject_is_not_recognition() {
        let mut l = PhoneLane::new(cfg(AsrMode::Interim));
        let (t, src, r) = l.inject_text("  thanks ").unwrap();
        assert_eq!((t.as_str(), src, r.as_str()), ("thanks", AsrSource::TextInject, "Any time."));
        assert_eq!(src.id(), "text-inject");
        assert!(!src.is_recognised());
        assert!(matches!(l.inject_text("   "), Err(PhoneError::Empty)));
        if !l.can_speak() {
            assert!(matches!(l.turn_text("hello"), Err(PhoneError::NoSpeech(_))));
            return;
        }
        let spoken = l.turn_text("hello").unwrap();
        assert_eq!(spoken.asr, AsrSource::TextInject);
        assert!(!spoken.asr.is_recognised());
        assert_eq!(spoken.reply_text, "Hi. I'm here — go ahead.");
        assert!(spoken.wav_bytes.starts_with(b"RIFF"));
    }
    #[test] fn s4_s5_reply_text_is_canned_english() {
        let mut l = PhoneLane::new(cfg(AsrMode::Stub));
        let (t, src, r) = l.reply_text(&fx("hello how are you")).unwrap();
        assert_eq!((t.as_str(), src, r.as_str()), ("hello how are you", AsrSource::StubFixture, "Hi! I'm doing well, thanks. How about you?"));
        let mut i = PhoneLane::new(cfg(AsrMode::Interim));
        assert_eq!(i.reply_text(&voice(0.5)).unwrap().2, "Hi. I'm here — go ahead.");
        let mut e = PhoneLane::new(PhoneConfig { asr: AsrMode::Stub, llm: LlmMode::Echo });
        assert_eq!(e.reply_text(&fx("hi")).unwrap().2, "you said: hi");
    }
    #[test] fn lane_is_send() { fn s<T: Send>() {} s::<PhoneLane>(); s::<PhoneTurn>(); s::<PhoneError>(); }
    /// Full S4→S5→S6 turn with real speech. Skips only if no speech backend exists.
    #[test] fn full_turn_emits_24k_speech_wav() {
        let mut l = PhoneLane::new(cfg(AsrMode::Interim));
        if !l.can_speak() {
            assert!(matches!(l.turn(&voice(1.5)), Err(PhoneError::NoSpeech(_))));
            eprintln!("SKIP: no speech backend: {}", l.speech_backend().1); return;
        }
        let t = l.turn(&voice(1.5)).unwrap();
        assert_eq!(t.transcript, "hello how are you");
        assert_eq!(t.reply_text, "Hi! I'm doing well, thanks. How about you?");
        assert_eq!(t.asr, AsrSource::InterimPlaceholder);
        assert!(t.llm.starts_with("canned-llm"));
        assert!(matches!(t.speech_backend, "a21-weights" | "system-flite"));
        let w = &t.wav_bytes;
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 24_000);
        let secs = (w.len() - 44) as f32 / 2.0 / 24_000.0;
        assert!((0.8..8.0).contains(&secs), "{secs}s");
        let peak = w[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs()).max().unwrap();
        assert!(peak > 1500, "speech too quiet: {peak}");
        assert!(t.ms >= t.stage_ms.tts);
        assert!(t.stage_ms.tts_first_pcm <= t.stage_ms.tts);
        let (pid, spawns) = l.piper_worker();
        // WAV-in path, same lane (stack and, when A21 is up, the same Piper process)
        let t2 = l.turn_wav(&wav_bytes(&fx("thanks"), 16_000)).unwrap();
        assert_eq!((t2.transcript.as_str(), t2.reply_text.as_str()), ("thanks", "Any time."));
        let (pid2, spawns2) = l.piper_worker();
        assert_eq!(pid2, pid, "piper pid changed across two turns");
        if pid.is_some() {
            assert_eq!(spawns, 1, "prewarm should be the only spawn");
            assert_eq!(spawns2, 1, "second turn must reuse the worker");
        }
    }
    #[test] fn speak_fixed_skips_the_canned_table() {
        let mut l = PhoneLane::with_speech(cfg(AsrMode::Stub), galion_ml_voice::speech::SpeechStack::silent("off"));
        assert!(matches!(l.speak_fixed("  "), Err(PhoneError::Empty)));
        let e = l.speak_fixed("Your mother died this morning.").unwrap_err();
        assert!(matches!(e, PhoneError::NoSpeech(_)), "{e}");
    }
    #[test] fn speech_off_is_not_a_spoken_reply() {
        let mut l = PhoneLane::with_speech(cfg(AsrMode::Interim), galion_ml_voice::speech::SpeechStack::silent("GALION_SPEECH=off"));
        let e = l.turn(&voice(1.5)).unwrap_err();
        assert!(matches!(e, PhoneError::NoSpeech(ref m) if m.contains("GALION_SPEECH=off")), "{e}");
        assert_eq!(l.piper_worker().0, None);
    }

    #[test]
    fn hermes_lane_uses_the_model_and_not_the_rule_table() {
        use crate::hermes::{HermesLlm, LLM_NAME};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let seen_w = seen.clone();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 8192];
            let mut got = Vec::new();
            loop {
                match sock.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        got.extend_from_slice(&buf[..n]);
                        if got.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                    }
                    Err(_) => break,
                }
            }
            if let Ok(s) = String::from_utf8(got) { *seen_w.lock().unwrap() = s; }
            let payload = "{\"choices\":[{\"message\":{\"content\":\"She died this morning.\"}}]}";
            let resp = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let mut l = PhoneLane::with_speech(
            PhoneConfig { asr: AsrMode::Stub, llm: LlmMode::Hermes },
            galion_ml_voice::speech::SpeechStack::silent("hermes-test"),
        );
        l.llm = Box::new(HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            LLM_NAME,
            "",
            Duration::from_millis(1000),
        ));
        l.set_persona("jarvis");
        let (t, src, r) = l.inject_text("My mother died this morning.").unwrap();
        assert_eq!(t, "My mother died this morning.");
        assert_eq!(src, AsrSource::TextInject);
        assert_eq!(r, "She died this morning.");
        assert_ne!(r, "Your mother died this morning.");
        assert!(!r.contains("doing well"));
        assert_eq!(l.llm.name(), LLM_NAME);
        let seen = seen.lock().unwrap().clone();
        assert!(seen.contains("Jarvis"), "{seen}");
    }
}
