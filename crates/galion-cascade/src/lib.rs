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
/// Loopback Hermes chat client (OpenAI-compatible, std `TcpStream`, no TLS).
pub mod hermes;
/// First sentence of a streaming reply, for TTS before the rest of the text.
mod early_reply;
/// Local BM25 answers for the research desk when Hermes does not answer.
pub mod offline;
/// Instruction-floor rule engine. Not the product brain. Unit tests only on the live path.
pub mod reflect;
/// S0 student `.gq` probe on galion-tensor. Blocked until a file can be spoken.
pub mod student;
/// Voice-stack cue that hands a spoken question to Galion Research.
pub mod research_handoff;
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
pub trait Llm {
    fn name(&self) -> &'static str;
    fn reply(&mut self, t: &Transcript, e: Emotion) -> Result<String, CascadeError>;
    /// Active test register (`samantha` or `jarvis`). The default ignores it.
    fn set_persona(&mut self, _id: &str) {}
    /// [`Self::reply`], delivering text as it arrives.
    ///
    /// The default waits for the whole reply and then calls `on_delta` once.
    /// Hermes overrides this and calls `on_delta` on each streamed piece.
    fn reply_deltas(
        &mut self,
        t: &Transcript,
        e: Emotion,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, CascadeError> {
        let text = self.reply(t, e)?;
        if !text.is_empty() {
            on_delta(&text);
        }
        Ok(text)
    }
}
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

pub struct Cascade<A, E, L, T> {
    pub asr: A,
    pub emo: E,
    pub llm: L,
    pub tts: T,
    /// Last turn's domain-04 sheet. Lengths and heartbeat times. No text, no PCM.
    pub last_stamps: galion_turn_stamps::TurnStamps,
}

impl<A: Asr, E: EmotionIn, L: Llm, T: Tts> Cascade<A, E, L, T> {
    /// ASR, the reply, and TTS. VAD and end-of-utterance stay absent: this lane
    /// does not decide them. The sheet records lengths, not the words or the samples.
    pub fn turn(&mut self, pcm: &[i16]) -> Result<Turn, CascadeError> {
        self.last_stamps = galion_turn_stamps::TurnStamps::dry();
        if pcm.is_empty() {
            return Err(CascadeError::Empty);
        }
        let mut clock = galion_turn_stamps::VirtualHeartbeat::new();
        let mut stamps = galion_turn_stamps::TurnStamps::dry();
        stamps.pcm_samples = galion_turn_stamps::sample_len(pcm.len());
        let transcript = match self.asr.transcribe(pcm) {
            Ok(t) => t,
            Err(e) => {
                self.last_stamps = stamps;
                return Err(e);
            }
        };
        stamps.t_asr_final = Some(clock.stamp());
        stamps.text_chars = galion_turn_stamps::char_len(&transcript.text);
        let emotion = self.emo.classify(pcm);
        stamps.t_llm_req = Some(clock.stamp());
        let mut first_token = None;
        let mut clause = None;
        let reply = match self.llm.reply_deltas(&transcript, emotion, &mut |delta| {
            if first_token.is_none() && !delta.is_empty() {
                first_token = Some(clock.stamp());
            }
            if clause.is_none() && clause_boundary(delta) {
                clause = Some(clock.stamp());
            }
        }) {
            Ok(text) => text,
            Err(e) => {
                stamps.t_llm_first_token = first_token;
                stamps.t_first_clause_flushed = clause;
                self.last_stamps = stamps;
                return Err(e);
            }
        };
        stamps.t_llm_first_token = first_token;
        if clause.is_none() && !reply.is_empty() {
            clause = Some(clock.stamp());
        }
        stamps.t_first_clause_flushed = clause;
        stamps.reply_chars = galion_turn_stamps::char_len(&reply);
        let audio = match self.tts.synth(&reply, transcript.lang) {
            Ok(pcm) => pcm,
            Err(e) => {
                self.last_stamps = stamps;
                return Err(e);
            }
        };
        if !audio.is_empty() {
            stamps.t_tts_first_pcm = Some(clock.stamp());
        }
        stamps.audio_samples = galion_turn_stamps::sample_len(audio.len());
        self.last_stamps = stamps;
        Ok(Turn { transcript, emotion, reply, audio })
    }
}

/// A clause is ready at punctuation. The characters are not stored.
fn clause_boundary(delta: &str) -> bool {
    delta.chars().any(|c| matches!(c, '.' | '!' | '?' | ',' | ';' | ':'))
}

pub fn mock_cascade() -> Cascade<MockAsr, MockEmotion, MockLlm, MockTts> {
    Cascade {
        asr: MockAsr,
        emo: MockEmotion,
        llm: MockLlm,
        tts: MockTts,
        last_stamps: galion_turn_stamps::TurnStamps::dry(),
    }
}

pub use canned::{CannedLlm, ProsodyEmotion, ReplyLlm};
pub use hermes::{contains_forbidden, HermesLlm, HermesTarget, FORBIDDEN, LLM_NAME, REFUSED, UNREACHABLE};
#[cfg(feature = "cascade")]
pub use phone::{
    phone_turn, phone_turn_wav, stub_fixture_pcm16, AsrMode, AsrSource, LlmMode, PhoneConfig, PhoneError, PhoneLane,
    PhoneTurn, PhoneTurnAsr,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn en_only() { assert_eq!(Lang::parse("en"), Some(Lang::En)); assert_eq!(Lang::parse("he-IL"), None); assert_eq!(Lang::parse("pl"), None); }
    #[test] fn mock_turn() { let t = mock_cascade().turn(&[100; 1600]).unwrap(); assert_eq!(t.emotion, Emotion::Neutral); assert!(t.reply.contains("hello galion")); assert!(!t.audio.is_empty()); }
    #[test] fn empty_rejected() { assert!(matches!(mock_cascade().turn(&[]), Err(CascadeError::Empty))); }
    #[test] fn loud_is_angry() { assert_eq!(MockEmotion.classify(&[20_000; 160]), Emotion::Angry); }
    #[test] fn budgets() { assert!(BARGE_IN_BUDGET_MS < ONSET_P50_BUDGET_MS); }

    #[test]
    fn mock_turn_stamps_lengths_not_words() {
        let mut lane = mock_cascade();
        let turn = lane.turn(&[100; 1600]).unwrap();
        let log = lane.last_stamps.log();
        assert!(galion_turn_stamps::log_is_lengths_only(&log));
        assert!(lane.last_stamps.t_vad_silence_start.is_none());
        assert!(lane.last_stamps.t_eou_decided.is_none());
        assert_eq!(lane.last_stamps.t_asr_final, Some(galion_turn_stamps::FRAME_MS));
        assert!(lane.last_stamps.t_llm_req.unwrap() > lane.last_stamps.t_asr_final.unwrap());
        assert!(lane.last_stamps.t_llm_first_token.is_some());
        assert!(lane.last_stamps.t_first_clause_flushed.is_some());
        assert!(lane.last_stamps.t_tts_first_pcm.is_some());
        assert_eq!(lane.last_stamps.pcm_samples, 1600);
        assert_eq!(lane.last_stamps.text_chars, galion_turn_stamps::char_len(&turn.transcript.text));
        assert_eq!(lane.last_stamps.reply_chars, galion_turn_stamps::char_len(&turn.reply));
        assert_eq!(lane.last_stamps.audio_samples, turn.audio.len() as u32);
        assert!(!log.contains(&turn.transcript.text));
        assert!(!log.contains(&turn.reply));
        assert!(!log.contains("hello"));
        assert!(!log.contains(' '));
    }

    #[test]
    fn empty_turn_leaves_the_sheet_dry() {
        let mut lane = mock_cascade();
        assert!(matches!(lane.turn(&[]), Err(CascadeError::Empty)));
        let log = lane.last_stamps.log();
        assert_eq!(log, galion_turn_stamps::dry_run_report());
        assert!(galion_turn_stamps::log_is_lengths_only(&log));
    }
}
