//! Galion Voice cascade fallback lane — stubs only (W20-10, sector 11).
//!
//! Pipeline: `Asr` (Whisper-class / Parakeet) -> `Llm` (small LLM, .gq only)
//! -> `Tts` (Piper / SpeechT5 / Pocket-TTS-class). `EmotionIn` is the
//! SenseVoice-Small inbound emotion reference slot. English only for now.
//! No model weights are loaded or downloaded; every backend here is a
//! deterministic mock so the harness exists before the model.

/// Budgets from A18 design.
/// W21-03: per-turn language route (pin > hint > LID), EN-only.
pub mod route;
/// W21-03: runnable EN demo lane on galion-ml-voice stubs (feature `cascade`).
#[cfg(feature = "cascade")]
pub mod en_lane;
/// Rule-table reply stand-in and prosody heuristic (no weights). Always compiled.
pub mod canned;
/// Phone turn: 16 kHz utterance → transcript → reply text → 24 kHz WAV (feature `cascade`).
#[cfg(feature = "cascade")]
pub mod phone;

pub const ONSET_P50_BUDGET_MS: u32 = 800;
pub const BARGE_IN_BUDGET_MS: u32 = 200;
pub const SAMPLE_RATE_HZ: u32 = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang { En }

impl Lang {
    /// Only EN is routed for now; anything else is rejected (no he-IL etc.).
    pub fn parse(tag: &str) -> Option<Lang> {
        match tag { "en" | "en-US" | "en-GB" => Some(Lang::En), _ => None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emotion { Neutral, Happy, Sad, Angry, Unknown }

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript { pub text: String, pub lang: Lang }

#[derive(Debug, Clone, PartialEq)]
pub enum CascadeError { Unsupported(&'static str), Empty, BudgetExceeded { stage: &'static str, ms: u32 } }

/// Inbound speech recognition (Whisper-class / Parakeet-class slot).
pub trait Asr { fn name(&self) -> &'static str; fn transcribe(&mut self, pcm: &[i16]) -> Result<Transcript, CascadeError>; }
/// Inbound emotion reference (SenseVoice-Small slot). Advisory only.
pub trait EmotionIn { fn classify(&mut self, pcm: &[i16]) -> Emotion; }
/// Small LLM reply (.gq only, no legacy container formats or third-party runtimes; gate G2 / X3).
pub trait Llm { fn name(&self) -> &'static str; fn reply(&mut self, t: &Transcript, e: Emotion) -> Result<String, CascadeError>; }
/// Outbound TTS (Piper / SpeechT5 / Pocket-TTS-class slot).
pub trait Tts { fn name(&self) -> &'static str; fn synth(&mut self, text: &str, lang: Lang) -> Result<Vec<i16>, CascadeError>; }

/// Mock ASR: returns a fixed EN transcript for non-empty audio.
pub struct MockAsr;
impl Asr for MockAsr {
    fn name(&self) -> &'static str { "mock-asr (parakeet/whisper slot)" }
    fn transcribe(&mut self, pcm: &[i16]) -> Result<Transcript, CascadeError> {
        if pcm.is_empty() { return Err(CascadeError::Empty); }
        Ok(Transcript { text: "hello galion".into(), lang: Lang::En })
    }
}
/// Mock emotion: RMS threshold placeholder until SenseVoice reference lands.
pub struct MockEmotion;
impl EmotionIn for MockEmotion {
    fn classify(&mut self, pcm: &[i16]) -> Emotion {
        if pcm.is_empty() { return Emotion::Unknown; }
        let ms = pcm.iter().map(|&s| (s as i64).pow(2)).sum::<i64>() / pcm.len() as i64;
        if ms > 8_000i64.pow(2) { Emotion::Angry } else { Emotion::Neutral }
    }
}
/// Mock LLM: deterministic echo.
pub struct MockLlm;
impl Llm for MockLlm {
    fn name(&self) -> &'static str { "mock-llm (small-llm slot, .gq only)" }
    fn reply(&mut self, t: &Transcript, e: Emotion) -> Result<String, CascadeError> {
        Ok(format!("[{e:?}] you said: {}", t.text))
    }
}
/// Mock TTS: 10 ms of silence per character.
pub struct MockTts;
impl Tts for MockTts {
    fn name(&self) -> &'static str { "mock-tts (piper/speecht5/pocket-tts slot)" }
    fn synth(&mut self, text: &str, _l: Lang) -> Result<Vec<i16>, CascadeError> {
        Ok(vec![0; text.chars().count() * (SAMPLE_RATE_HZ as usize / 100)])
    }
}

pub struct Turn { pub transcript: Transcript, pub emotion: Emotion, pub reply: String, pub audio: Vec<i16> }

pub struct Cascade<A, E, L, T> { pub asr: A, pub emo: E, pub llm: L, pub tts: T }

impl<A: Asr, E: EmotionIn, L: Llm, T: Tts> Cascade<A, E, L, T> {
    pub fn turn(&mut self, pcm: &[i16]) -> Result<Turn, CascadeError> {
        let transcript = self.asr.transcribe(pcm)?;
        let emotion = self.emo.classify(pcm);
        let reply = self.llm.reply(&transcript, emotion)?;
        let audio = self.tts.synth(&reply, transcript.lang)?;
        Ok(Turn { transcript, emotion, reply, audio })
    }
}

pub fn mock_cascade() -> Cascade<MockAsr, MockEmotion, MockLlm, MockTts> {
    Cascade { asr: MockAsr, emo: MockEmotion, llm: MockLlm, tts: MockTts }
}

pub use canned::{CannedLlm, ProsodyEmotion, ReplyLlm};
#[cfg(feature = "cascade")]
pub use phone::{
    phone_turn, phone_turn_wav, AsrMode, AsrSource, LlmMode, PhoneConfig, PhoneError, PhoneLane, PhoneTurn, PhoneTurnAsr,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn en_only() { assert_eq!(Lang::parse("en"), Some(Lang::En)); assert_eq!(Lang::parse("he-IL"), None); assert_eq!(Lang::parse("pl"), None); }
    #[test] fn mock_turn() { let t = mock_cascade().turn(&[100; 1600]).unwrap(); assert_eq!(t.emotion, Emotion::Neutral); assert!(t.reply.contains("hello galion")); assert!(!t.audio.is_empty()); }
    #[test] fn empty_rejected() { assert!(matches!(mock_cascade().turn(&[]), Err(CascadeError::Empty))); }
    #[test] fn loud_is_angry() { assert_eq!(MockEmotion.classify(&[20_000; 160]), Emotion::Angry); }
    #[test] fn budgets() { assert!(BARGE_IN_BUDGET_MS < ONSET_P50_BUDGET_MS); }
}
