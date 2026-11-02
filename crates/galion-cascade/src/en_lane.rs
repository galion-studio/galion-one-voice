// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! W21-03: runnable ENGLISH demo lane (behind feature `cascade`).
//!
//! ASR-stub (galion-ml-voice `StubAsr`) -> route (pin > hint > LID) ->
//! MockEmotion -> MockLlm reply -> TTS-stub (`StubTts`) -> WAV.
//! Two renders of the reply:
//! - `fixture`: the exact StubTts signal (decodable back to text, loop check);
//! - `audible`: a deterministic earcon, one 80 ms frame per character
//!   (LOCKS `[clock] frame_ms = 80`), so the live demo has something to hear.
//!
//! Rate note: the stub lane runs at 24 kHz end to end (Mimi-class bus). The
//! 16 kHz ASR-slot resampler is a later step (PROTOTYPE.md "Next steps").
//! No weights, no I/O except the caller writing the WAV bytes.
//!
//! V1-03 (FRXDEL Q2: the EN reply must say actual words): a third render,
//! `speech`, speaks the reply text through `galion_ml_voice::speech::SpeechStack`
//! (A21 weight slot if active, else interim system libflite). The earcon is kept
//! only as a labelled fallback/diagnostic; it is never presented as the reply.

use crate::route::{CascadeRoute, RouteDecision};
use crate::*;
use galion_ml_voice::stub::{fixture_audio, StubAsr, StubTts};
use galion_ml_voice::{with_lang_token, Audio, AsrEngine, LanguageId, TtsEngine, VoiceError};

/// Lane sample rate (24 kHz, Mimi-class).
pub const LANE_RATE_HZ: u32 = galion_ml_voice::stub::SAMPLE_RATE;
/// One heartbeat frame at the lane rate: 80 ms @ 24 kHz.
pub const FRAME_SAMPLES: usize = (LANE_RATE_HZ as usize) * 80 / 1000;

fn to_ml(l: Lang) -> LanguageId { match l { Lang::En => LanguageId::En } }
fn map_err(e: VoiceError) -> CascadeError {
    match e { VoiceError::EmptyInput => CascadeError::Empty, VoiceError::UnsupportedLanguage(_) => CascadeError::Unsupported("language not enabled (EN-only build)"), VoiceError::Backend(_) => CascadeError::Unsupported("speech backend unavailable") }
}
/// f32 [-1,1] -> PCM16 (clamped).
pub fn to_pcm16(s: &[f32]) -> Vec<i16> { s.iter().map(|x| (x.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).collect() }
/// PCM16 -> f32.
pub fn to_f32(s: &[i16]) -> Vec<f32> { s.iter().map(|&x| x as f32 / i16::MAX as f32).collect() }

/// Adapter: any ml-voice `AsrEngine` as a cascade `Asr`.
pub struct MlAsr<A: AsrEngine>(pub A);
impl<A: AsrEngine> Asr for MlAsr<A> {
    fn name(&self) -> &'static str { "ml-voice StubAsr (EN, 24 kHz)" }
    fn transcribe(&mut self, pcm: &[i16]) -> Result<Transcript, CascadeError> {
        let a = Audio { sample_rate: LANE_RATE_HZ, samples: to_f32(pcm) };
        let t = self.0.transcribe(&a, Some(LanguageId::En)).map_err(map_err)?;
        let _ = t.lang; // EN only.
        Ok(Transcript { text: t.text, lang: Lang::En })
    }
}
/// Adapter: any ml-voice `TtsEngine` as a cascade `Tts` (reply tagged `<|en|>`).
pub struct MlTts<T: TtsEngine>(pub T);
impl<T: TtsEngine> Tts for MlTts<T> {
    fn name(&self) -> &'static str { "ml-voice StubTts (EN, 24 kHz)" }
    fn synth(&mut self, text: &str, lang: Lang) -> Result<Vec<i16>, CascadeError> {
        let tagged = with_lang_token(to_ml(lang), text);
        Ok(to_pcm16(&self.0.synthesize(&tagged, to_ml(lang)).map_err(map_err)?.samples))
    }
}

/// Emotion slot for the demo lane. The canned mic signal is a DC code, not
/// speech, so RMS-based `MockEmotion` would mis-read it as Angry; the lane
/// takes a fixed (operator-chosen) emotion instead. Advisory only.
pub struct FixedEmotion(pub Emotion);
impl EmotionIn for FixedEmotion { fn classify(&mut self, _pcm: &[i16]) -> Emotion { self.0 } }

/// Parse an emotion name for the demo CLI.
pub fn parse_emotion(s: &str) -> Option<Emotion> {
    match s.trim().to_ascii_lowercase().as_str() {
        "neutral" => Some(Emotion::Neutral), "happy" => Some(Emotion::Happy), "sad" => Some(Emotion::Sad), "angry" => Some(Emotion::Angry), _ => None,
    }
}

/// The EN lane cascade type.
pub type EnCascade = Cascade<MlAsr<StubAsr>, FixedEmotion, MockLlm, MlTts<StubTts>>;
pub fn en_cascade(emotion: Emotion) -> EnCascade { Cascade { asr: MlAsr(StubAsr), emo: FixedEmotion(emotion), llm: MockLlm, tts: MlTts(StubTts) } }

/// One finished demo turn.
pub struct DemoTurn {
    pub route: RouteDecision,
    pub turn: Turn,
    /// Exact StubTts render (PCM16 @ 24 kHz).
    pub fixture: Vec<i16>,
    /// Earcon render, frame-aligned (PCM16 @ 24 kHz). Diagnostic only; NOT speech.
    pub audible: Vec<i16>,
    /// V1-03: the reply as spoken English words (PCM16 @ 24 kHz), `None` if no
    /// speech backend is available on this machine.
    pub speech: Option<Vec<i16>>,
    /// The exact text that was spoken (reply minus the `[Emotion]` tag).
    pub spoken_text: String,
    /// Speech backend id (`a21-weights` | `system-flite` | `none`) + human label.
    pub speech_backend: (&'static str, String),
    /// A21 weight-slot status id + summary (never claims weights that are not there).
    pub a21_weights: (&'static str, String),
}

/// Reply text as it should be spoken: drops the leading `[Emotion]` tag.
pub fn spoken_text(reply: &str) -> String {
    let r = reply.trim();
    match (r.starts_with('['), r.find(']')) { (true, Some(i)) => r[i + 1..].trim().to_string(), _ => r.to_string() }
}

/// Speak `text` with the detected speech stack; `(pcm, backend, weights)`.
pub fn speak(text: &str) -> (Option<Vec<i16>>, (&'static str, String), (&'static str, String)) {
    let mut st = galion_ml_voice::speech::SpeechStack::detect();
    let pcm = if st.can_speak() { st.speak(text).ok().map(|a| { debug_assert_eq!(a.sample_rate, LANE_RATE_HZ); to_pcm16(&a.samples) }) } else { None };
    let backend = if pcm.is_some() { (st.backend.id(), st.backend.label()) } else { ("none", st.backend.label()) };
    (pcm, backend, (st.weights.id(), st.weights.summary()))
}

/// Run one EN turn from canned input text. `pin`/`hint` follow the P0.2 contract.
pub fn run_en_turn(input_text: &str, pin: Option<&str>, hint: Option<&str>) -> Result<DemoTurn, CascadeError> {
    run_en_turn_with(input_text, pin, hint, Emotion::Neutral)
}

/// As [`run_en_turn`] with an explicit emotion for the feeling slot.
pub fn run_en_turn_with(input_text: &str, pin: Option<&str>, hint: Option<&str>, emotion: Emotion) -> Result<DemoTurn, CascadeError> {
    let mic = to_pcm16(&fixture_audio(input_text).samples);
    let route = CascadeRoute::default().route(pin, hint, &mic)?;
    if mic.is_empty() { return Err(CascadeError::Empty); }
    let mut c = en_cascade(emotion);
    let mut turn = c.turn(&mic)?;
    turn.transcript.lang = route.lang;
    let fixture = turn.audio.clone();
    let audible = earcon(&turn.reply);
    let spoken = spoken_text(&turn.reply);
    let (speech, speech_backend, a21_weights) = speak(&spoken);
    Ok(DemoTurn { route, turn, fixture, audible, speech, spoken_text: spoken, speech_backend, a21_weights })
}

/// Deterministic audible render: one 80 ms frame per char (60 ms tone with
/// 5 ms ramps + 20 ms gap); spaces/punctuation are silent frames; one silent
/// frame lead-in and lead-out. Letters map onto a C-major pentatonic ladder.
pub fn earcon(text: &str) -> Vec<i16> {
    const PENTA: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
    let tone = FRAME_SAMPLES * 3 / 4;
    let ramp = LANE_RATE_HZ as usize * 5 / 1000;
    let mut out = vec![0i16; FRAME_SAMPLES];
    for ch in text.chars() {
        let mut frame = vec![0i16; FRAME_SAMPLES];
        if ch.is_ascii_alphanumeric() {
            let idx = (ch.to_ascii_lowercase() as u32 % 15) as usize;
            let semis = PENTA[idx % 5] + 12.0 * (idx / 5) as f32;
            let f = 261.63_f32 * 2f32.powf(semis / 12.0);
            for (i, s) in frame.iter_mut().take(tone).enumerate() {
                let env = (i.min(tone - 1 - i) as f32 / ramp as f32).min(1.0);
                let ph = 2.0 * core::f32::consts::PI * f * i as f32 / LANE_RATE_HZ as f32;
                *s = (0.3 * env * ph.sin() * i16::MAX as f32).round() as i16;
            }
        }
        out.extend_from_slice(&frame);
    }
    out.extend(core::iter::repeat(0).take(FRAME_SAMPLES));
    out
}

/// Mono PCM16 little-endian RIFF/WAVE bytes.
pub fn wav_bytes(pcm: &[i16], rate: u32) -> Vec<u8> {
    let n = (pcm.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + n as usize);
    v.extend_from_slice(b"RIFF"); v.extend_from_slice(&(36 + n).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt "); v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes()); v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes()); v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data"); v.extend_from_slice(&n.to_le_bytes());
    for s in pcm { v.extend_from_slice(&s.to_le_bytes()); }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::RouteSource;
    #[test] fn en_turn_end_to_end() {
        let d = run_en_turn("hello galion", None, None).unwrap();
        assert_eq!(d.turn.transcript.text, "hello galion");
        assert_eq!(d.turn.reply, "[Neutral] you said: hello galion");
        assert_eq!(d.route.source, RouteSource::Lid);
        assert_eq!(d.fixture.len(), d.turn.reply.len() * galion_ml_voice::stub::SAMPLES_PER_CHAR);
    }
    #[test] fn tts_loop_closes() {
        let d = run_en_turn("hello galion", Some("en"), None).unwrap();
        let back = MlAsr(StubAsr).transcribe(&d.fixture).unwrap();
        assert_eq!(back.text, d.turn.reply);
    }
    #[test] fn audible_is_frame_aligned_and_nonsilent() {
        let d = run_en_turn("hello galion", None, Some("en-US")).unwrap();
        assert_eq!(d.audible.len() % FRAME_SAMPLES, 0);
        assert_eq!(d.audible.len() / FRAME_SAMPLES, d.turn.reply.chars().count() + 2);
        let peak = d.audible.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 8_000 && peak < 11_000, "peak {peak}");
        assert!(d.audible[..FRAME_SAMPLES].iter().all(|&s| s == 0));
    }
    #[test] fn deterministic() { assert_eq!(run_en_turn("hi", None, None).unwrap().audible, run_en_turn("hi", None, None).unwrap().audible); }
    #[test] fn non_en_pin_rejected() { for p in ["he-IL", "pl"] { assert!(matches!(run_en_turn("hi", Some(p), None), Err(CascadeError::Unsupported(_)))); } }
    #[test] fn non_en_hint_ignored() { assert_eq!(run_en_turn("hi", None, Some("pl")).unwrap().route.source, RouteSource::Lid); }
    #[test] fn empty_rejected() { assert!(matches!(run_en_turn("", None, None), Err(CascadeError::Empty))); assert!(matches!(run_en_turn("", Some("en"), None), Err(CascadeError::Empty))); }
    #[test] fn wav_header() {
        let w = wav_bytes(&[0, 1, -1], LANE_RATE_HZ);
        assert_eq!(&w[0..4], b"RIFF"); assert_eq!(&w[36..40], b"data");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 24_000);
        assert_eq!(w.len(), 44 + 6);
    }
    #[test] fn emotion_slot() {
        assert_eq!(run_en_turn_with("hi", None, None, Emotion::Happy).unwrap().turn.reply, "[Happy] you said: hi");
        assert_eq!(parse_emotion(" SAD "), Some(Emotion::Sad)); assert_eq!(parse_emotion("x"), None);
    }
    #[test] fn frame_is_80ms() { assert_eq!(FRAME_SAMPLES, 1920); }
    #[test] fn spoken_text_drops_tag() {
        assert_eq!(spoken_text("[Neutral] you said: hello galion"), "you said: hello galion");
        assert_eq!(spoken_text("plain"), "plain");
    }
    #[test] fn a21_slot_never_pretends() {
        let d = run_en_turn("hello galion", None, None).unwrap();
        // FP-03: A21 speaks only when the pack is on disk, sha256-verified and the Piper runtime exists.
        if d.speech_backend.0 == "a21-weights" { assert_eq!(d.a21_weights.0, "present-verified", "{}", d.a21_weights.1); }
        if d.a21_weights.0 != "present-verified" { assert_ne!(d.speech_backend.0, "a21-weights"); }
        if d.a21_weights.0 == "absent" { assert!(d.a21_weights.1.contains("ABSENT")); }
    }
    /// V1-03 / FRXDEL Q2: reply must be speech (words), not a tone ladder.
    /// Skips (with a message) only when no speech backend exists on the machine.
    #[test] fn reply_is_speech_not_tones() {
        let d = run_en_turn("hello galion", None, None).unwrap();
        let Some(sp) = d.speech.as_ref() else { eprintln!("SKIP: no speech backend: {}", d.speech_backend.1); return; };
        let ms = sp.len() * 1000 / LANE_RATE_HZ as usize;
        assert!((900..6000).contains(&ms), "speech length {ms} ms");
        assert_ne!(sp, &d.audible);
        // Speech alternates voiced (low ZCR) and unvoiced/fricative (high ZCR) 20 ms
        // windows; a pentatonic earcon has near-constant ZCR per tone.
        let w = LANE_RATE_HZ as usize / 50;
        let zcr: Vec<f32> = sp.chunks(w).filter(|c| c.len() == w && c.iter().map(|&x| (x as i32).abs()).max().unwrap() > 1500)
            .map(|c| c.windows(2).filter(|p| (p[0] >= 0) != (p[1] >= 0)).count() as f32 / w as f32).collect();
        assert!(zcr.len() > 20, "too few voiced windows: {}", zcr.len());
        let (lo, hi) = zcr.iter().fold((1f32, 0f32), |(a, b), &z| (a.min(z), b.max(z)));
        assert!(hi > 3.0 * lo.max(0.005), "ZCR range too flat for speech: {lo}..{hi}");
        // deterministic
        assert_eq!(run_en_turn("hello galion", None, None).unwrap().speech, d.speech);
    }
}
