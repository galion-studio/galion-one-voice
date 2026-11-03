// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V1-03 real-speech path (feature `speech`). ENGLISH ONLY.
//!
//! Selection order (first that can actually speak wins):
//! 1. [`weights::A21WeightTts`] — the A21 neural TTS slot (FP-03). Activates when a
//!    sha256-verified voice pack is on disk (`GALION_TTS_WEIGHTS` /
//!    `GALION_SPEECH_WEIGHTS` or `<repo>/weights/tts/`) AND a decoder for it exists
//!    ([`weights::decoder_for`]). Today: Piper VITS `en_US-ljspeech-medium` (MIT),
//!    decoded on the CPU by one persistent out-of-process Piper worker
//!    ([`piper_worker`]). With no pack it reports `Absent` and the stack falls
//!    back to flite.
//! 2. [`flite::FliteTts`] — FALLBACK system TTS (only when the A21 pack/runtime is
//!    missing): the distro `libflite1`, opened with `dlopen`. Real words, not neural.
//! 3. none — the caller must fail loudly (FRXDEL Q2: the EN cascade must say
//!    actual words; tones are not an acceptable reply).
//!
//! Env: `GALION_SPEECH=auto|a21|flite|off` (default `auto`: a21, else flite),
//!      `GALION_A21_RUNTIME` (Piper exe), `GALION_A21_NOISE=natural` (sampled prosody),
//!      `GALION_FLITE_VOICE=slt|kal16|awb|rms|kal` (default `slt`).

use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub mod clause;
pub mod flite;
pub mod piper_worker;
pub mod sha256;
pub mod weights;

pub use clause::{ClauseBuf, CLAUSE_WORDS};

/// Output rate of every speech backend (Mimi-class 24 kHz lane bus).
pub const SPEECH_RATE: u32 = crate::stub::SAMPLE_RATE;

/// Which backend produced (or would produce) speech.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeechBackend {
    /// A21 neural TTS (Piper VITS voice pack, CPU, out of process).
    A21Weights { voice: String },
    /// Interim system TTS (libflite, runtime dlopen). Real words, not neural.
    SystemFlite { voice: String },
    /// Nothing can speak on this machine.
    None { reason: String },
}

impl SpeechBackend {
    /// Stable short id for JSON / UI labels.
    pub fn id(&self) -> &'static str {
        match self {
            SpeechBackend::A21Weights { .. } => "a21-weights",
            SpeechBackend::SystemFlite { .. } => "system-flite",
            SpeechBackend::None { .. } => "none",
        }
    }
    /// Gate/runbook token: `SPEECH=a21|flite|ABSENT`.
    pub fn speech_token(&self) -> &'static str {
        match self { SpeechBackend::A21Weights { .. } => "a21", SpeechBackend::SystemFlite { .. } => "flite", SpeechBackend::None { .. } => "ABSENT" }
    }
    /// Human label; always states what it is NOT.
    pub fn label(&self) -> String {
        match self {
            SpeechBackend::A21Weights { voice } => format!("A21 neural TTS (Piper VITS {voice}; CPU; MIT)"),
            SpeechBackend::SystemFlite { voice } => format!("fallback system TTS (libflite cmu_us_{voice}; real words; NOT A21, NOT neural)"),
            SpeechBackend::None { reason } => format!("no speech backend ({reason})"),
        }
    }
}

/// Cold/warm first-clause PCM for one stack. Times are absent until that speak happens.
/// No PCM is stored here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warmth {
    /// First utterance on this stack, speak entry → first clause PCM.
    pub cold_first_pcm_ms: Option<u32>,
    /// Second utterance, same process, speak entry → first clause PCM.
    pub warm_first_pcm_ms: Option<u32>,
    /// First-clause PCM of the utterance that just finished.
    pub last_first_pcm_ms: Option<u32>,
    pub piper_pid: Option<u32>,
    /// `1` after a normal A21 session: the process was not started again.
    pub piper_spawns: u32,
}

/// The selected speech engine plus an honest status report.
pub struct SpeechStack {
    pub backend: SpeechBackend,
    pub weights: weights::WeightStatus,
    engine: Option<Engine>,
    prewarmed: bool,
    utterances: u32,
    cold_first_pcm_ms: Option<u32>,
    warm_first_pcm_ms: Option<u32>,
    last_first_pcm_ms: Option<u32>,
    /// `turn_id < cancel` means the caller superseded this utterance.
    cancel: Option<Arc<AtomicU64>>,
    turn_id: u64,
}

enum Engine {
    A21(weights::A21WeightTts),
    Flite(flite::FliteTts),
    #[cfg(test)]
    Mock(Box<dyn TtsEngine + Send>),
}

impl SpeechStack {
    fn bare(backend: SpeechBackend, weights: weights::WeightStatus, engine: Option<Engine>) -> Self {
        Self {
            backend, weights, engine, prewarmed: false, utterances: 0,
            cold_first_pcm_ms: None, warm_first_pcm_ms: None, last_first_pcm_ms: None,
            cancel: None, turn_id: 0,
        }
    }

    /// Detect from env + disk. Never downloads anything.
    /// Does not pre-warm; the phone lane calls [`prewarm`](Self::prewarm) once.
    pub fn detect() -> Self {
        let mode = std::env::var("GALION_SPEECH").unwrap_or_else(|_| "auto".into()).to_ascii_lowercase();
        let weights = weights::WeightStatus::discover();
        if mode == "off" {
            return Self::bare(SpeechBackend::None { reason: "GALION_SPEECH=off".into() }, weights, None);
        }
        if mode == "auto" || mode == "a21" {
            if let Ok(t) = weights::A21WeightTts::try_new(&weights) {
                let voice = t.voice().to_string();
                return Self::bare(SpeechBackend::A21Weights { voice }, weights, Some(Engine::A21(t)));
            }
            if mode == "a21" {
                let reason = format!("GALION_SPEECH=a21 but A21 slot inactive: {}", weights.summary());
                return Self::bare(SpeechBackend::None { reason }, weights, None);
            }
        }
        if mode == "auto" || mode == "flite" { match flite::FliteTts::open_default() {
            Ok(t) => {
                let voice = t.voice().to_string();
                Self::bare(SpeechBackend::SystemFlite { voice }, weights, Some(Engine::Flite(t)))
            }
            Err(e) => Self::bare(SpeechBackend::None { reason: format!("{e}; {}", weights.summary()) }, weights, None),
        } } else {
            Self::bare(SpeechBackend::None { reason: format!("unknown GALION_SPEECH={mode} (auto|a21|flite|off)") }, weights, None)
        }
    }

    /// A stack that cannot speak. `GALION_SPEECH=off` uses this shape.
    pub fn silent(reason: impl Into<String>) -> Self {
        let weights = weights::WeightStatus::discover();
        Self::bare(SpeechBackend::None { reason: reason.into() }, weights, None)
    }

    /// True if this stack can produce spoken words.
    pub fn can_speak(&self) -> bool { self.engine.is_some() }

    /// Discard one short synthesis so the first caller turn is not the cold start.
    /// No PCM is kept. Flite and `off` do nothing.
    pub fn prewarm(&mut self) {
        if self.prewarmed || !self.is_piper() { return; }
        if self.speak("Hello.").is_ok() { self.prewarmed = true; }
    }

    /// Supersede gate shared with the turn worker. A stale `turn_id` drops
    /// clauses that have not been sent yet. The Piper process stays up.
    pub fn arm_cancel(&mut self, current: Arc<AtomicU64>, turn_id: u64) {
        self.cancel = Some(current);
        self.turn_id = turn_id;
    }

    pub fn warmth(&self) -> Warmth {
        let (piper_pid, piper_spawns) = match &self.engine {
            Some(Engine::A21(t)) => (t.piper_pid(), t.piper_spawns()),
            _ => (None, 0),
        };
        Warmth {
            cold_first_pcm_ms: self.cold_first_pcm_ms,
            warm_first_pcm_ms: self.warm_first_pcm_ms,
            last_first_pcm_ms: self.last_first_pcm_ms,
            piper_pid, piper_spawns,
        }
    }

    /// Speak `text` (English) at [`SPEECH_RATE`].
    ///
    /// On Piper, clauses flush at punctuation or [`CLAUSE_WORDS`] words so the
    /// first PCM does not wait for the rest of the sentence. Flite speaks the
    /// whole text in one call.
    pub fn speak(&mut self, text: &str) -> Result<Audio, VoiceError> {
        let mut stream = self.begin_stream();
        stream.push(text)?;
        stream.finish()
    }

    /// Streaming text. Piper synthesizes a clause as soon as [`push`](SpeechStream::push)
    /// closes it. Flite waits for [`finish`](SpeechStream::finish).
    pub fn begin_stream(&mut self) -> SpeechStream<'_> {
        SpeechStream {
            piper: self.is_piper(),
            stack: self,
            clauses: ClauseBuf::new(),
            flite_buf: String::new(),
            samples: Vec::new(),
            rate: SPEECH_RATE,
            started: Instant::now(),
            first_pcm_ms: None,
        }
    }

    fn is_piper(&self) -> bool { matches!(self.backend, SpeechBackend::A21Weights { .. }) }

    fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|c| self.turn_id < c.load(Ordering::Acquire))
    }

    fn note_utterance(&mut self, first_pcm_ms: u32) {
        self.utterances += 1;
        self.last_first_pcm_ms = Some(first_pcm_ms);
        if self.utterances == 1 { self.cold_first_pcm_ms = Some(first_pcm_ms); }
        else if self.utterances == 2 { self.warm_first_pcm_ms = Some(first_pcm_ms); }
    }

    fn synth_one(&mut self, text: &str) -> Result<Audio, VoiceError> {
        if self.cancelled() { return Err(VoiceError::Cancelled); }
        match self.engine.as_mut() {
            None => Err(VoiceError::Backend(self.backend.label())),
            Some(Engine::Flite(t)) => t.synthesize(text, LanguageId::En),
            #[cfg(test)]
            Some(Engine::Mock(t)) => t.synthesize(text, LanguageId::En),
            Some(Engine::A21(t)) => {
                let first = t.synthesize(text, LanguageId::En);
                if first.is_ok() || t.worker_alive() { return first; }
                t.respawn()?;
                t.synthesize(text, LanguageId::En)
            }
        }
    }

    #[cfg(test)]
    fn from_mock(engine: Box<dyn TtsEngine + Send>) -> Self {
        let weights = weights::WeightStatus::Absent { searched: Vec::new() };
        Self::bare(SpeechBackend::A21Weights { voice: "mock".into() }, weights, Some(Engine::Mock(engine)))
    }
}

/// One utterance fed as text arrives.
pub struct SpeechStream<'a> {
    stack: &'a mut SpeechStack,
    piper: bool,
    clauses: ClauseBuf,
    flite_buf: String,
    samples: Vec<f32>,
    rate: u32,
    started: Instant,
    first_pcm_ms: Option<u32>,
}

impl SpeechStream<'_> {
    pub fn push(&mut self, delta: &str) -> Result<(), VoiceError> {
        if delta.is_empty() { return Ok(()); }
        if self.piper {
            for clause in self.clauses.push(delta) { self.synth(&clause)?; }
        } else {
            self.flite_buf.push_str(delta);
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Audio, VoiceError> {
        if self.piper {
            for clause in self.clauses.finish() { self.synth(&clause)?; }
        } else {
            let text = std::mem::take(&mut self.flite_buf);
            let text = text.trim();
            if !text.is_empty() { self.synth(text)?; }
        }
        if self.samples.is_empty() { return Err(VoiceError::EmptyInput); }
        let ms = self.first_pcm_ms.unwrap_or_else(|| elapsed_ms(self.started));
        self.stack.note_utterance(ms);
        Ok(Audio { sample_rate: self.rate, samples: self.samples })
    }

    fn synth(&mut self, clause: &str) -> Result<(), VoiceError> {
        if clause.trim().is_empty() { return Ok(()); }
        let audio = self.stack.synth_one(clause)?;
        if self.first_pcm_ms.is_none() { self.first_pcm_ms = Some(elapsed_ms(self.started)); }
        if self.samples.is_empty() || audio.sample_rate == self.rate {
            self.rate = audio.sample_rate;
            self.samples.extend(audio.samples);
        } else {
            self.samples.extend(resample(&audio.samples, audio.sample_rate, self.rate));
        }
        Ok(())
    }
}

fn elapsed_ms(t0: Instant) -> u32 { t0.elapsed().as_millis().min(u32::MAX as u128) as u32 }

/// Cubic (Catmull-Rom) resampler, mono. Deterministic; adequate for speech
/// up-sampling 16 kHz -> 24 kHz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() { return input.to_vec(); }
    let n_out = ((input.len() as u64 * to as u64) / from as u64) as usize;
    let at = |i: isize| -> f32 { input[i.clamp(0, input.len() as isize - 1) as usize] };
    (0..n_out)
        .map(|k| {
            let pos = k as f64 * from as f64 / to as f64;
            let i = pos.floor() as isize;
            let t = (pos - i as f64) as f32;
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
            let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c = -0.5 * p0 + 0.5 * p2;
            ((a * t + b) * t + c) * t + p1
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    #[test] fn resample_len_and_identity() {
        let x: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(resample(&x, 16_000, 24_000).len(), 2400);
        assert_eq!(resample(&x, 24_000, 24_000), x);
    }
    #[test] fn off_mode_never_speaks() {
        // Pure function check on the label path (env is process-global; not mutated here).
        let b = SpeechBackend::None { reason: "x".into() };
        assert_eq!(b.id(), "none");
        assert!(SpeechBackend::SystemFlite { voice: "slt".into() }.label().contains("NOT A21"));
        let mut off = SpeechStack::silent("GALION_SPEECH=off");
        assert!(!off.can_speak());
        assert!(off.speak("hello").is_err());
        off.prewarm();
        assert_eq!(off.warmth().piper_pid, None);
    }

    #[test]
    fn stream_flushes_first_clause_before_the_rest() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut s = SpeechStack::from_mock(Box::new(LogTts { log: log.clone(), gate: None }));
        let mut stream = s.begin_stream();
        stream.push("Hel").unwrap();
        assert!(log.lock().unwrap().is_empty(), "open word must wait");
        stream.push("lo! and then the rest of the reply").unwrap();
        assert_eq!(log.lock().unwrap().as_slice(), ["Hello!"]);
        let audio = stream.finish().unwrap();
        assert_eq!(log.lock().unwrap().as_slice(), ["Hello!", "and then the rest of the reply"]);
        assert!(!audio.samples.is_empty());
        assert!(s.warmth().last_first_pcm_ms.is_some());
    }

    #[test]
    fn cancel_between_clauses_leaves_the_stack_usable() {
        let gate = std::sync::Arc::new(AtomicU64::new(0));
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut s = SpeechStack::from_mock(Box::new(LogTts { log: log.clone(), gate: Some(gate.clone()) }));
        s.arm_cancel(gate, 1);
        let err = s.speak("Hi! This second clause should not be sent.");
        assert!(matches!(err, Err(VoiceError::Cancelled)), "{err:?}");
        assert_eq!(log.lock().unwrap().as_slice(), ["Hi!"]);
        s.arm_cancel(std::sync::Arc::new(AtomicU64::new(0)), 1);
        s.speak("Thanks.").unwrap();
        assert_eq!(log.lock().unwrap().as_slice(), ["Hi!", "Thanks."]);
    }

    struct LogTts {
        log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        gate: Option<std::sync::Arc<AtomicU64>>,
    }
    impl TtsEngine for LogTts {
        fn languages(&self) -> &[LanguageId] { LanguageId::ALL }
        fn synthesize(&mut self, text: &str, _: LanguageId) -> Result<Audio, VoiceError> {
            self.log.lock().unwrap().push(text.to_string());
            if let Some(g) = &self.gate { g.store(99, Ordering::Release); }
            Ok(Audio { sample_rate: SPEECH_RATE, samples: vec![0.2; 48] })
        }
    }
}
