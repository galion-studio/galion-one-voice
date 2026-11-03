// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! S4+S5+S6 · **phone turn**: 16 kHz PCM/WAV in → transcript → reply text → 24 kHz
//! spoken WAV out (feature `cascade`). ENGLISH ONLY, CPU, fallback lane (never the
//! 80 ms primary path).
//!
//! ```text
//!  pcm16k ──S4 PhoneTurnAsr──▶ transcript ──S5 CannedLlm──▶ reply_text ──S6 SpeechStack──▶ wav_bytes (24 kHz)
//! ```
//!
//! # ⚠ S4 ASR IS NOT RATIFIED — READ THIS
//! Real English ASR on the phone path is **blocked on an owner decision**:
//! see `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md` (options **A** Vosk out of process,
//! **B** extend A21 ONNX exception to ASR, **C** hold the stub until native
//! Whisper-tiny.en `.safetensors` runs on galion-tensor). whisper.cpp / GGML is
//! explicitly rejected. Until the owner picks, [`PhoneTurnAsr`] has exactly three modes:
//!
//! | `GALION_ASR`     | behaviour |
//! |------------------|-----------|
//! | unset / `stub`   | **default.** Decodes `galion_ml_voice::stub` fixture audio via `StubAsr` (tests, demos). Real speech → [`PhoneError::AsrUnratified`]. |
//! | `interim`        | **INTERIM PLACEHOLDER — NOT SPEECH RECOGNITION.** Any non-silent audio yields a short canned English transcript chosen by utterance length. It does not listen to the words. Exists only so the phone loop produces real Piper WAV end to end. Fixtures still decode exactly. |
//! | `off`            | Always [`PhoneError::AsrUnratified`]. |
//!
//! Every [`PhoneTurn`] carries `asr` so a UI can label an interim transcript honestly.
//! When the owner ratifies A/B/C, the real engine replaces the `Interim` arm only.
//!
//! # S5 LLM
//! Phone turns default to [`CannedLlm`] (`GALION_LLM=canned` or unset). `GALION_LLM=echo`
//! selects the old `MockLlm` echo. Emotion is fixed `Neutral`: inferring affect from
//! the caller's voice is the LOCKS `[consent] sense` bit (default deny).
//!
//! # S6 TTS
//! [`SpeechStack::detect`] (A21 Piper VITS `en_US-ljspeech-medium` if the sha256-verified
//! pack + Piper runtime are on disk, else libflite). No backend → [`PhoneError::NoSpeech`];
//! tones are never returned as a reply (FRXDEL Q2).
//!
//! # Threads
//! [`PhoneLane`] is `Send`: build it once on a worker thread and call [`PhoneLane::turn`]
//! per utterance (the speech stack is reused). [`phone_turn`] / [`phone_turn_wav`] are
//! one-shot conveniences that read env each call.

use crate::canned::CannedLlm;
use crate::en_lane::{spoken_text, to_f32, to_pcm16, wav_bytes, LANE_RATE_HZ};
use crate::{CascadeError, Emotion, Lang, Llm, MockLlm, Transcript};
use galion_ml_voice::speech::{resample, SpeechStack};
use galion_ml_voice::stub::SAMPLES_PER_CHAR;
use std::time::Instant;

/// Phone-path input rate (16 kHz mono PCM16).
pub const PHONE_IN_RATE_HZ: u32 = crate::SAMPLE_RATE_HZ;
/// Output WAV rate (24 kHz mono PCM16, Mimi-class lane bus).
pub const PHONE_OUT_RATE_HZ: u32 = LANE_RATE_HZ;
/// Pointer embedded in every "not ratified" error.
pub const RATIFY_DOC: &str = "docs/policy/RATIFY-EN-ASR-PHONE-PATH.md";

/// S4 ASR mode (see module docs; `GALION_ASR`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrMode { Stub, Interim, Off }

impl AsrMode {
    pub fn from_env() -> Self { Self::from_choice(std::env::var("GALION_ASR").ok().as_deref()) }
    pub fn from_choice(v: Option<&str>) -> Self {
        match v.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("interim") => AsrMode::Interim,
            Some("off") => AsrMode::Off,
            _ => AsrMode::Stub,
        }
    }
}

/// S5 LLM mode (`GALION_LLM`). Phone default is `Canned`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmMode { Canned, Echo }

impl LlmMode {
    pub fn from_env() -> Self { Self::from_choice(std::env::var("GALION_LLM").ok().as_deref()) }
    pub fn from_choice(v: Option<&str>) -> Self {
        match v.map(|s| s.trim().to_ascii_lowercase()).as_deref() { Some("echo") | Some("mock") => LlmMode::Echo, _ => LlmMode::Canned }
    }
}

/// Phone-turn configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhoneConfig { pub asr: AsrMode, pub llm: LlmMode }

impl PhoneConfig {
    /// `GALION_ASR` + `GALION_LLM`.
    pub fn from_env() -> Self { Self { asr: AsrMode::from_env(), llm: LlmMode::from_env() } }
}
impl Default for PhoneConfig { fn default() -> Self { Self { asr: AsrMode::Stub, llm: LlmMode::Canned } } }

/// Phone-turn failure.
#[derive(Debug, Clone, PartialEq)]
pub enum PhoneError {
    /// No audio, or only silence.
    Empty,
    /// WAV bytes could not be parsed (only PCM16 RIFF is accepted).
    BadWav(String),
    /// Real speech arrived but no ratified ASR exists (see [`RATIFY_DOC`]).
    AsrUnratified(&'static str),
    /// No speech backend on this machine (Piper/flite missing); never falls back to tones.
    NoSpeech(String),
    /// Error from the cascade (LLM / route).
    Cascade(CascadeError),
}

impl std::fmt::Display for PhoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhoneError::Empty => write!(f, "empty or silent audio"),
            PhoneError::BadWav(m) => write!(f, "bad WAV: {m}"),
            PhoneError::AsrUnratified(m) => write!(f, "{m}"),
            PhoneError::NoSpeech(m) => write!(f, "no speech backend: {m}"),
            PhoneError::Cascade(e) => write!(f, "cascade: {e:?}"),
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
}
impl AsrSource {
    pub fn id(&self) -> &'static str {
        match self {
            AsrSource::StubFixture => "stub-fixture",
            AsrSource::InterimPlaceholder => "interim-placeholder",
            AsrSource::TextInject => "text-inject",
        }
    }
    /// True only for an exact stub-fixture decode.
    /// Interim and text-inject are not recognised speech.
    pub fn is_recognised(&self) -> bool { matches!(self, AsrSource::StubFixture) }
}

/// Per-stage wall time (ms).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StageMs { pub asr: u32, pub llm: u32, pub tts: u32 }

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
#[derive(Debug, Clone, Copy)]
pub struct PhoneTurnAsr { pub mode: AsrMode }

/// RMS (0..1) below which the input counts as silence.
pub const SILENCE_RMS: f32 = 0.003;

/// Interim placeholders by utterance length (short → long). NOT recognition.
pub const INTERIM_TRANSCRIPTS: [&str; 3] = ["hello", "hello how are you", "can you hear me"];

impl PhoneTurnAsr {
    pub fn new(mode: AsrMode) -> Self { Self { mode } }

    pub fn transcribe(&self, pcm16k: &[i16]) -> Result<(String, AsrSource), PhoneError> {
        if pcm16k.is_empty() { return Err(PhoneError::Empty); }
        if self.mode == AsrMode::Off {
            return Err(PhoneError::AsrUnratified("EN phone ASR disabled (GALION_ASR=off); see docs/policy/RATIFY-EN-ASR-PHONE-PATH.md options A/B/C"));
        }
        if let Some(t) = decode_stub_fixture(pcm16k) { return Ok((t, AsrSource::StubFixture)); }
        let rms = (pcm16k.iter().map(|&s| (s as f32 / 32768.0).powi(2)).sum::<f32>() / pcm16k.len() as f32).sqrt();
        if rms < SILENCE_RMS { return Err(PhoneError::Empty); }
        match self.mode {
            AsrMode::Interim => {
                // ⚠ INTERIM PLACEHOLDER — the words are NOT recognised (see module docs).
                let secs = pcm16k.len() as f32 / PHONE_IN_RATE_HZ as f32;
                let i = if secs < 1.0 { 0 } else if secs < 2.5 { 1 } else { 2 };
                Ok((INTERIM_TRANSCRIPTS[i].to_string(), AsrSource::InterimPlaceholder))
            }
            _ => Err(PhoneError::AsrUnratified(
                "EN phone ASR not ratified: real speech cannot be transcribed yet. Owner must pick A/B/C in docs/policy/RATIFY-EN-ASR-PHONE-PATH.md; set GALION_ASR=interim for the placeholder loop",
            )),
        }
    }
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
    pub fn from_env() -> Self { Self::new(PhoneConfig::from_env()) }

    pub fn new(cfg: PhoneConfig) -> Self {
        let llm: Box<dyn Llm + Send> = match cfg.llm { LlmMode::Canned => Box::new(CannedLlm::new()), LlmMode::Echo => Box::new(MockLlm) };
        Self { cfg, asr: PhoneTurnAsr::new(cfg.asr), llm, speech: SpeechStack::detect() }
    }

    /// True when S6 can produce spoken words on this machine.
    pub fn can_speak(&self) -> bool { self.speech.can_speak() }
    /// `(id, label)` of the speech backend.
    pub fn speech_backend(&self) -> (&'static str, String) { (self.speech.backend.id(), self.speech.backend.label()) }

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
        let audio = self.speech.speak(&reply_text).map_err(|e| PhoneError::NoSpeech(format!("{e:?}")))?;
        let pcm = to_pcm16(&resample(&audio.samples, audio.sample_rate, PHONE_OUT_RATE_HZ));
        let wav = wav_bytes(&pcm, PHONE_OUT_RATE_HZ);
        let tts_ms = ms_since(t2);
        Ok(PhoneTurn {
            transcript, reply_text, wav_bytes: wav, ms: ms_since(t0),
            stage_ms: StageMs { asr: asr_ms, llm: llm_ms, tts: tts_ms }, asr,
            llm: self.llm.name(), speech_backend: self.speech.backend.id(), speech_label: self.speech.backend.label(),
        })
    }

    /// As [`turn`](Self::turn) from WAV bytes (PCM16, any rate; resampled to 16 kHz).
    pub fn turn_wav(&mut self, wav: &[u8]) -> Result<PhoneTurn, PhoneError> { self.turn(&wav_to_pcm16k(wav)?) }
}

/// One-shot phone turn from 16 kHz mono PCM16 (env-configured).
pub fn phone_turn(pcm16k: &[i16]) -> Result<PhoneTurn, PhoneError> { PhoneLane::from_env().turn(pcm16k) }
/// One-shot phone turn from WAV bytes (env-configured).
pub fn phone_turn_wav(wav: &[u8]) -> Result<PhoneTurn, PhoneError> { PhoneLane::from_env().turn_wav(wav) }

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

    #[test] fn env_choices() {
        assert_eq!(AsrMode::from_choice(None), AsrMode::Stub);
        assert_eq!(AsrMode::from_choice(Some(" Interim ")), AsrMode::Interim);
        assert_eq!(AsrMode::from_choice(Some("whisper")), AsrMode::Stub);
        assert_eq!(LlmMode::from_choice(None), LlmMode::Canned);
        assert_eq!(LlmMode::from_choice(Some("canned")), LlmMode::Canned);
        assert_eq!(LlmMode::from_choice(Some("echo")), LlmMode::Echo);
    }
    #[test] fn stub_decodes_fixture_in_every_mode_but_off() {
        for m in [AsrMode::Stub, AsrMode::Interim] {
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&fx("how are you")).unwrap(), ("how are you".into(), AsrSource::StubFixture));
        }
        assert!(matches!(PhoneTurnAsr::new(AsrMode::Off).transcribe(&fx("hi")), Err(PhoneError::AsrUnratified(_))));
    }
    #[test] fn default_refuses_real_speech_and_points_at_policy() {
        let e = PhoneTurnAsr::new(AsrMode::Stub).transcribe(&voice(1.5)).unwrap_err();
        assert!(matches!(e, PhoneError::AsrUnratified(m) if m.contains(RATIFY_DOC) && m.contains("A/B/C")));
    }
    #[test] fn interim_is_labelled_placeholder() {
        let a = PhoneTurnAsr::new(AsrMode::Interim);
        assert_eq!(a.transcribe(&voice(0.5)).unwrap(), ("hello".into(), AsrSource::InterimPlaceholder));
        assert_eq!(a.transcribe(&voice(1.5)).unwrap().0, "hello how are you");
        assert_eq!(a.transcribe(&voice(4.0)).unwrap().0, "can you hear me");
        assert!(!AsrSource::InterimPlaceholder.is_recognised());
    }
    #[test] fn silence_and_empty_are_empty() {
        for m in [AsrMode::Stub, AsrMode::Interim] {
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&[]), Err(PhoneError::Empty));
            assert_eq!(PhoneTurnAsr::new(m).transcribe(&vec![0; 16_000]), Err(PhoneError::Empty));
        }
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
        // WAV-in path, same lane (stack reused)
        let t2 = l.turn_wav(&wav_bytes(&fx("thanks"), 16_000)).unwrap();
        assert_eq!((t2.transcript.as_str(), t2.reply_text.as_str()), ("thanks", "Any time."));
    }
}
