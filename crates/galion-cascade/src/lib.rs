//! Galion Voice cascade fallback lane — stubs only (W20-10, sector 11).
//!
//! Pipeline: `Asr` (Whisper-class / Parakeet) -> `Llm` (small LLM, .gq only)
//! -> `Tts` (Piper / SpeechT5 / Pocket-TTS-class). `EmotionIn` is the
//! SenseVoice-Small inbound emotion reference slot. English only for now.
//! No model weights are loaded or downloaded; every backend here is a
//! deterministic mock so the harness exists before the model.

/// W21-03: per-turn language route (pin > hint > LID), EN-only.
pub mod route;
/// W21-03: runnable EN demo lane on galion-ml-voice stubs (feature `cascade`).
#[cfg(feature = "cascade")]
pub mod en_lane;
/// Rule-table reply stand-in and prosody heuristic (no weights). Always compiled.
pub mod canned;
/// HORBM heuristic pick for a chat turn. The canned table stays for `GALION_LLM=canned`.
pub mod horbm_chat;
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
/// Domain 04 monotonic stamps. Lengths and timings only.
pub mod stamps;
/// Phone turn: 16 kHz utterance → transcript → reply text → 24 kHz WAV (feature `cascade`).
#[cfg(feature = "cascade")]
pub mod phone;
/// Unix speech workers, or a stub that does not spawn (feature `cascade`).
#[cfg(feature = "cascade")]
pub mod speech_worker;

/// Onset p50 ceiling, milliseconds. `LOCKS.toml` `[gates] onset_p50_max_ms`.
/// The lock file is the source of truth. A test parses it and checks this
/// constant. A measurement that misses the number does not change it.
pub const ONSET_P50_BUDGET_MS: u32 = 300;
/// Barge-in ceiling, milliseconds. `LOCKS.toml` `[gates] barge_in_max_ms`.
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
pub use horbm_chat::HorbmChat;
pub use stamps::{dry_run_log, stamp_log_clean, Domain04Stamps, StampClock, StampPoint};
pub use hermes::{contains_forbidden, HermesLlm, HermesTarget, FORBIDDEN, LLM_NAME, REFUSED, UNREACHABLE};
#[cfg(feature = "cascade")]
pub use phone::{
    phone_turn, phone_turn_wav, stub_fixture_pcm16, AsrMode, AsrSource, FailNote, LlmMode, PhoneConfig, PhoneError,
    PhoneLane, PhoneTurn, PhoneTurnAsr,
};

/// Nearest-rank percentile. Same index as `galion_presence::percentile`:
/// `ceil(pct/100 * n)`, then that rank minus one. `sorted` must be sorted.
#[cfg(test)]
pub(crate) fn nearest_rank(sorted: &[u64], pct: u64) -> u64 {
    assert!(!sorted.is_empty() && (1..=100).contains(&pct));
    let rank = (pct * sorted.len() as u64).div_ceil(100).max(1) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// Wall-clock p50 of 49 turns after 16 warmup turns. The closure is one turn.
#[cfg(test)]
pub(crate) fn onset_p50_us(mut turn: impl FnMut()) -> u64 {
    for _ in 0..16 {
        turn();
    }
    let mut samples = Vec::with_capacity(49);
    for _ in 0..49 {
        let t0 = std::time::Instant::now();
        turn();
        samples.push(u64::try_from(t0.elapsed().as_micros()).unwrap_or(u64::MAX));
    }
    samples.sort_unstable();
    nearest_rank(&samples, 50)
}

/// The lock is the ceiling. A miss names the measured gap and does not pass.
#[cfg(test)]
pub(crate) fn assert_onset_within_lock(p50_us: u64, lane: &str) {
    let budget_us = u64::from(ONSET_P50_BUDGET_MS) * 1_000;
    eprintln!("S61 {lane} onset_p50_us={p50_us} budget_us={budget_us}");
    assert!(p50_us > 0, "{lane} clock did not move");
    assert!(
        p50_us <= budget_us,
        "{lane} onset p50 {p50_us} µs exceeds LOCKS onset_p50_max_ms {budget} ms by {gap} µs. Not a pass. The lock stays {budget}.",
        budget = ONSET_P50_BUDGET_MS,
        gap = p50_us.saturating_sub(budget_us),
    );
}

/// Integer after `=` in `[gates]`, ignoring a trailing comment.
#[cfg(test)]
fn lock_gate_ms(key: &str) -> u32 {
    let locks = include_str!("../../../LOCKS.toml");
    let mut in_gates = false;
    for line in locks.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_gates = trimmed == "[gates]";
            continue;
        }
        if !in_gates || trimmed.starts_with('#') {
            continue;
        }
        let Some((k, rest)) = trimmed.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let num = rest.split('#').next().unwrap_or("").trim();
        return num.parse().unwrap_or_else(|_| panic!("[gates] {key} is not an integer"));
    }
    panic!("LOCKS.toml [gates] is missing {key}");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn en_only() { assert_eq!(Lang::parse("en"), Some(Lang::En)); assert_eq!(Lang::parse("he-IL"), None); assert_eq!(Lang::parse("pl"), None); }
    #[test] fn mock_turn() { let t = mock_cascade().turn(&[100; 1600]).unwrap(); assert_eq!(t.emotion, Emotion::Neutral); assert!(t.reply.contains("hello galion")); assert!(!t.audio.is_empty()); }
    #[test] fn empty_rejected() { assert!(matches!(mock_cascade().turn(&[]), Err(CascadeError::Empty))); }
    #[test] fn loud_is_angry() { assert_eq!(MockEmotion.classify(&[20_000; 160]), Emotion::Angry); }

    #[test]
    fn budgets_match_locks_toml() {
        let onset = lock_gate_ms("onset_p50_max_ms");
        let barge = lock_gate_ms("barge_in_max_ms");
        assert_eq!(ONSET_P50_BUDGET_MS, onset);
        assert_eq!(BARGE_IN_BUDGET_MS, barge);
        assert!(barge < onset, "barge-in ceiling {barge} ms is not under onset {onset} ms");
    }

    /// Stub lane: `MockAsr` → `MockLlm` → `MockTts`. Not the phone speech path.
    /// p50 is this process. A miss fails with the gap. The constant stays the lock.
    #[test]
    fn stub_lane_onset_p50_against_the_lock() {
        let pcm = [100i16; 1600];
        let mut lane = mock_cascade();
        let p50 = onset_p50_us(|| {
            let turn = lane.turn(&pcm).expect("stub turn");
            assert!(!turn.audio.is_empty());
            assert!(turn.audio.iter().all(|&s| s == 0), "stub tts is silence, not pcm words");
        });
        assert_onset_within_lock(p50, "stub lane");
    }
}
