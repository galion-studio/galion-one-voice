// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V1-03 real-speech path (feature `speech`). ENGLISH ONLY.
//!
//! Selection order (first that can actually speak wins):
//! 0. [`clone_worker::CloneTts`] — V2-S14 Chatterbox slot, out of process, only when
//!    the pinned safetensors are on disk. Nano primary, Turbo via `GALION_CLONE_MODEL`.
//!    Absent weights skip this slot (no download). A dead child falls back to A21.
//!    Worker stdout is raw s16le. That is not the runtime format. [`gqvoice`] compiles
//!    each landed utterance into `voices/gq/<persona>.gqvoice` and this slot plays
//!    the packet it just wrote.
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
//! Env: `GALION_SPEECH=auto|clone|a21|flite|off` (default `auto`: clone if the
//!      pinned pack is on disk, else a21, else flite),
//!      `GALION_CLONE_MODEL=nano|turbo` (nano stays the landing model),
//!      `GALION_CLONE_PERSONA=samantha|jarvis`,
//!      `GALION_CLIP_STORE` (optional dir of `{persona}/reference.wav`),
//!      `GALION_CLONE_THREADS` (clamped to 1; this child is one vCPU),
//!      `GALION_A21_RUNTIME` (Piper exe), `GALION_A21_NOISE=natural` (sampled prosody),
//!      `GALION_FLITE_VOICE=slt|kal16|awb|rms|kal` (default `slt`).
//! The clone child is CPU-only. It does not see a GPU and it does not load GGUF.

use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub mod clause;
pub mod clone_worker;
pub mod galion_clip;
pub mod flite;
pub mod gqvoice;
pub mod piper_worker;
pub mod sha256;
pub mod weights;

pub use clause::{ClauseBuf, CLAUSE_WORDS};

/// Output rate of every speech backend (Mimi-class 24 kHz lane bus).
pub const SPEECH_RATE: u32 = crate::stub::SAMPLE_RATE;

/// Which backend produced (or would produce) speech.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeechBackend {
    /// V2-S14 clone worker (Chatterbox Nano or Turbo, CPU, out of process).
    /// Not a film voice. Piper speaks if this child is absent.
    Clone { model: String, persona: String },
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
            SpeechBackend::Clone { .. } => "clone-worker",
            SpeechBackend::A21Weights { .. } => "a21-weights",
            SpeechBackend::SystemFlite { .. } => "system-flite",
            SpeechBackend::None { .. } => "none",
        }
    }
    /// Gate/runbook token: `SPEECH=clone|a21|flite|ABSENT`.
    pub fn speech_token(&self) -> &'static str {
        match self {
            SpeechBackend::Clone { .. } => "clone",
            SpeechBackend::A21Weights { .. } => "a21",
            SpeechBackend::SystemFlite { .. } => "flite",
            SpeechBackend::None { .. } => "ABSENT",
        }
    }
    /// Human label; always states what it is NOT.
    pub fn label(&self) -> String {
        match self {
            SpeechBackend::Clone { model, persona } => format!("V2-S14 clone worker (Chatterbox {model}, register {persona}; CPU; out of process; not a film voice)"),
            SpeechBackend::A21Weights { voice } => format!("A21 neural TTS (Piper VITS {voice}; CPU; MIT)"),
            SpeechBackend::SystemFlite { voice } => format!("fallback system TTS (libflite cmu_us_{voice}; real words; NOT A21, NOT neural)"),
            SpeechBackend::None { reason } => format!("no speech backend ({reason})"),
        }
    }
}

/// Throwaway lines spoken (and discarded) by [`SpeechStack::prewarm`].
pub const PREWARM_LINES: &[&str] = &["Hello.", "Okay, I hear you.", "That sounds good, tell me more about it."];

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
    pub clone_pid: Option<u32>,
    /// `1` after a normal clone session: the process was not started again.
    pub clone_spawns: u32,
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
    Clone(clone_worker::CloneTts),
    A21(weights::A21WeightTts),
    Flite(flite::FliteTts),
    #[cfg(test)]
    Mock(Box<dyn TtsEngine + Send>),
}

impl SpeechStack {
    fn bare(
        backend: SpeechBackend,
        weights: weights::WeightStatus,
        engine: Option<Engine>,
    ) -> Self {
        Self {
            backend,
            weights,
            engine,
            prewarmed: false,
            utterances: 0,
            cold_first_pcm_ms: None,
            warm_first_pcm_ms: None,
            last_first_pcm_ms: None,
            cancel: None,
            turn_id: 0,
        }
    }

    /// Detect from env + disk. Never downloads anything.
    /// Does not pre-warm; the phone lane calls [`prewarm`](Self::prewarm) once.
    pub fn detect() -> Self {
        let mode = std::env::var("GALION_SPEECH")
            .unwrap_or_else(|_| "auto".into())
            .to_ascii_lowercase();
        let weights = weights::WeightStatus::discover();
        if mode == "off" {
            return Self::bare(
                SpeechBackend::None {
                    reason: "GALION_SPEECH=off".into(),
                },
                weights,
                None,
            );
        }
        if mode == "auto" || mode == "clone" {
            if let Ok(t) = clone_worker::CloneTts::try_new() {
                let model = t.model_name().to_string();
                let persona = t.persona().to_string();
                return Self::bare(
                    SpeechBackend::Clone { model, persona },
                    weights,
                    Some(Engine::Clone(t)),
                );
            }
        }
        if mode == "auto" || mode == "a21" || mode == "clone" {
            if let Ok(t) = weights::A21WeightTts::try_new(&weights) {
                let voice = t.voice().to_string();
                return Self::bare(
                    SpeechBackend::A21Weights { voice },
                    weights,
                    Some(Engine::A21(t)),
                );
            }
            if mode == "a21" {
                let reason = format!(
                    "GALION_SPEECH=a21 but A21 slot inactive: {}",
                    weights.summary()
                );
                return Self::bare(SpeechBackend::None { reason }, weights, None);
            }
        }
        if mode == "auto" || mode == "flite" || mode == "clone" {
            match flite::FliteTts::open_default() {
                Ok(t) => {
                    let voice = t.voice().to_string();
                    Self::bare(
                        SpeechBackend::SystemFlite { voice },
                        weights,
                        Some(Engine::Flite(t)),
                    )
                }
                Err(e) => Self::bare(
                    SpeechBackend::None {
                        reason: format!("{e}; {}", weights.summary()),
                    },
                    weights,
                    None,
                ),
            }
        } else {
            Self::bare(
                SpeechBackend::None {
                    reason: format!("unknown GALION_SPEECH={mode} (auto|clone|a21|flite|off)"),
                },
                weights,
                None,
            )
        }
    }

    /// A stack that cannot speak. `GALION_SPEECH=off` uses this shape.
    pub fn silent(reason: impl Into<String>) -> Self {
        let weights = weights::WeightStatus::discover();
        Self::bare(
            SpeechBackend::None {
                reason: reason.into(),
            },
            weights,
            None,
        )
    }

    /// True if this stack can produce spoken words.
    pub fn can_speak(&self) -> bool {
        self.engine.is_some()
    }

    /// Discard one short synthesis so the first caller turn is not the cold start.
    /// No PCM is kept. Flite and `off` do nothing.
    pub fn prewarm(&mut self) {
        if self.prewarmed || !self.streams_clauses() {
            return;
        }
        // lat240: three lines of different lengths at call start. The first
        // pays process spawn + model load (cold, ~330 ms); the next two take the
        // first-run cost of new input lengths so no caller turn sees either.
        let mut ok = false;
        for line in PREWARM_LINES {
            ok |= self.speak(line).is_ok();
        }
        self.prewarmed = ok;
    }

    /// Supersede gate shared with the turn worker. A stale `turn_id` drops
    /// clauses that have not been sent yet. The Piper process stays up.
    pub fn arm_cancel(&mut self, current: Arc<AtomicU64>, turn_id: u64) {
        self.cancel = Some(current);
        self.turn_id = turn_id;
    }

    pub fn warmth(&self) -> Warmth {
        let (piper_pid, piper_spawns, clone_pid, clone_spawns) = match &self.engine {
            Some(Engine::A21(t)) => (t.piper_pid(), t.piper_spawns(), None, 0),
            Some(Engine::Clone(t)) => (None, 0, t.clone_pid(), t.clone_spawns()),
            _ => (None, 0, None, 0),
        };
        Warmth {
            cold_first_pcm_ms: self.cold_first_pcm_ms,
            warm_first_pcm_ms: self.warm_first_pcm_ms,
            last_first_pcm_ms: self.last_first_pcm_ms,
            piper_pid,
            piper_spawns,
            clone_pid,
            clone_spawns,
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
        self.clear_clone_pending();
        SpeechStream {
            piper: self.streams_clauses(),
            stack: self,
            clauses: ClauseBuf::new(),
            flite_buf: String::new(),
            samples: Vec::new(),
            rate: SPEECH_RATE,
            started: Instant::now(),
            first_pcm_ms: None,
        }
    }

    fn streams_clauses(&self) -> bool {
        matches!(
            self.backend,
            SpeechBackend::A21Weights { .. } | SpeechBackend::Clone { .. }
        )
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| self.turn_id < c.load(Ordering::Acquire))
    }

    fn note_utterance(&mut self, first_pcm_ms: u32) {
        self.utterances += 1;
        self.last_first_pcm_ms = Some(first_pcm_ms);
        if self.utterances == 1 {
            self.cold_first_pcm_ms = Some(first_pcm_ms);
        } else if self.utterances == 2 {
            self.warm_first_pcm_ms = Some(first_pcm_ms);
        }
    }

    fn synth_one(&mut self, text: &str) -> Result<Audio, VoiceError> {
        if self.cancelled() {
            return Err(VoiceError::Cancelled);
        }
        let Some(mut engine) = self.engine.take() else {
            return Err(VoiceError::Backend(self.backend.label()));
        };
        let result = match &mut engine {
            Engine::Flite(t) => t.synthesize(text, LanguageId::En),
            #[cfg(test)]
            Engine::Mock(t) => t.synthesize(text, LanguageId::En),
            Engine::A21(t) => {
                let first = t.synthesize(text, LanguageId::En);
                if first.is_ok() || t.worker_alive() {
                    first
                } else {
                    match t.respawn() {
                        Ok(()) => t.synthesize(text, LanguageId::En),
                        Err(e) => Err(e),
                    }
                }
            }
            Engine::Clone(t) => t.synthesize(text, LanguageId::En),
        };
        let failed_clone = matches!(engine, Engine::Clone(_))
            && result.is_err()
            && !matches!(result, Err(VoiceError::Cancelled));
        self.engine = Some(engine);
        if failed_clone {
            self.fallback_piper(text)
        } else {
            result
        }
    }

    fn clear_clone_pending(&mut self) {
        if let Some(Engine::Clone(t)) = self.engine.as_mut() {
            t.clear_pending();
        }
    }

    /// Write the finished clone utterance as `.gqvoice` and return the samples
    /// loaded back from that file. Piper and flite do not land a packet.
    fn land_clone(&mut self) -> Result<Option<Audio>, VoiceError> {
        match self.engine.as_mut() {
            Some(Engine::Clone(t)) => t.land_pending(),
            _ => Ok(None),
        }
    }

    /// Piper speaks when the clone child cannot. The lane is not restarted.
    fn fallback_piper(&mut self, text: &str) -> Result<Audio, VoiceError> {
        self.clear_clone_pending();
        let Ok(piper) = weights::A21WeightTts::try_new(&self.weights) else {
            return Err(VoiceError::Backend(
                "clone worker failed and A21 piper is absent".into(),
            ));
        };
        let voice = piper.voice().to_string();
        self.backend = SpeechBackend::A21Weights { voice };
        self.engine = Some(Engine::A21(piper));
        self.synth_one(text)
    }

    #[cfg(test)]
    fn from_mock(engine: Box<dyn TtsEngine + Send>) -> Self {
        let weights = weights::WeightStatus::Absent {
            searched: Vec::new(),
        };
        Self::bare(
            SpeechBackend::A21Weights {
                voice: "mock".into(),
            },
            weights,
            Some(Engine::Mock(engine)),
        )
    }

    #[cfg(test)]
    fn from_clone(engine: clone_worker::CloneTts) -> Self {
        let weights = weights::WeightStatus::Absent {
            searched: Vec::new(),
        };
        let backend = SpeechBackend::Clone {
            model: engine.model_name().to_string(),
            persona: engine.persona().to_string(),
        };
        Self::bare(backend, weights, Some(Engine::Clone(engine)))
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
        if delta.is_empty() {
            return Ok(());
        }
        if self.piper {
            for clause in self.clauses.push(delta) {
                self.synth(&clause)?;
            }
        } else {
            self.flite_buf.push_str(delta);
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Audio, VoiceError> {
        if self.piper {
            for clause in self.clauses.finish() {
                self.synth(&clause)?;
            }
        } else {
            let text = std::mem::take(&mut self.flite_buf);
            let text = text.trim();
            if !text.is_empty() {
                self.synth(text)?;
            }
        }
        if self.samples.is_empty() {
            return Err(VoiceError::EmptyInput);
        }
        if let Some(landed) = self.stack.land_clone()? {
            if landed.sample_rate != self.rate || landed.samples != self.samples {
                return Err(VoiceError::Backend(
                    "gqvoice: playback buffer is not the landed packet".into(),
                ));
            }
        }
        let ms = self
            .first_pcm_ms
            .unwrap_or_else(|| elapsed_ms(self.started));
        self.stack.note_utterance(ms);
        Ok(Audio {
            sample_rate: self.rate,
            samples: self.samples,
        })
    }

    fn synth(&mut self, clause: &str) -> Result<(), VoiceError> {
        if clause.trim().is_empty() {
            return Ok(());
        }
        let audio = self.stack.synth_one(clause)?;
        if self.first_pcm_ms.is_none() {
            self.first_pcm_ms = Some(elapsed_ms(self.started));
        }
        if self.samples.is_empty() || audio.sample_rate == self.rate {
            self.rate = audio.sample_rate;
            self.samples.extend(audio.samples);
        } else {
            self.samples
                .extend(resample(&audio.samples, audio.sample_rate, self.rate));
        }
        Ok(())
    }
}

fn elapsed_ms(t0: Instant) -> u32 {
    t0.elapsed().as_millis().min(u32::MAX as u128) as u32
}

/// Cubic (Catmull-Rom) resampler, mono. Deterministic; adequate for speech
/// up-sampling 16 kHz -> 24 kHz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
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
    #[test]
    fn resample_len_and_identity() {
        let x: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(resample(&x, 16_000, 24_000).len(), 2400);
        assert_eq!(resample(&x, 24_000, 24_000), x);
    }
    #[test]
    fn off_mode_never_speaks() {
        // Pure function check on the label path (env is process-global; not mutated here).
        let b = SpeechBackend::None { reason: "x".into() };
        assert_eq!(b.id(), "none");
        assert!(SpeechBackend::SystemFlite {
            voice: "slt".into()
        }
        .label()
        .contains("NOT A21"));
        let mut off = SpeechStack::silent("GALION_SPEECH=off");
        assert!(!off.can_speak());
        assert!(off.speak("hello").is_err());
        off.prewarm();
        assert_eq!(off.warmth().piper_pid, None);
    }

    #[test]
    fn stream_flushes_first_clause_before_the_rest() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut s = SpeechStack::from_mock(Box::new(LogTts {
            log: log.clone(),
            gate: None,
        }));
        let mut stream = s.begin_stream();
        stream.push("Hel").unwrap();
        assert!(log.lock().unwrap().is_empty(), "open word must wait");
        stream.push("lo! and then the rest of the reply").unwrap();
        assert_eq!(log.lock().unwrap().as_slice(), ["Hello!"]);
        let audio = stream.finish().unwrap();
        assert_eq!(
            log.lock().unwrap().as_slice(),
            ["Hello!", "and then the rest of the reply"]
        );
        assert!(!audio.samples.is_empty());
        assert!(s.warmth().last_first_pcm_ms.is_some());
    }

    #[test]
    fn cancel_between_clauses_leaves_the_stack_usable() {
        let gate = std::sync::Arc::new(AtomicU64::new(0));
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut s = SpeechStack::from_mock(Box::new(LogTts {
            log: log.clone(),
            gate: Some(gate.clone()),
        }));
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
        fn languages(&self) -> &[LanguageId] {
            LanguageId::ALL
        }
        fn synthesize(&mut self, text: &str, _: LanguageId) -> Result<Audio, VoiceError> {
            self.log.lock().unwrap().push(text.to_string());
            if let Some(g) = &self.gate {
                g.store(99, Ordering::Release);
            }
            Ok(Audio {
                sample_rate: SPEECH_RATE,
                samples: vec![0.2; 48],
            })
        }
    }

    #[test]
    fn landed_clone_is_compiled_before_the_stack_plays_it() {
        let dir = std::env::temp_dir().join(format!("galion-gqvoice-stack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, FAKE_CLONE).unwrap();
        let worker =
            clone_worker::CloneWorker::spawn(&script, clone_worker::CloneModel::Nano, "samantha")
                .unwrap();
        let mut stack =
            SpeechStack::from_clone(clone_worker::CloneTts::for_test(worker, dir.clone()));
        let both = stack.speak("Hello! Go.").unwrap();
        let path = dir.join("samantha.gqvoice");
        let packet = gqvoice::GqVoice::open(&path).unwrap();
        assert_eq!(packet.persona_name(), "samantha");
        assert_eq!(packet.model_name(), "nano");
        assert!(packet.as_bytes().starts_with(&gqvoice::MAGIC));
        assert_eq!(both.samples, packet.to_audio().samples);
        assert_eq!(both.samples.len(), 402, "both clauses, pad not played");
        assert_eq!(both.samples[0], 6.0 / 32768.0);
        assert_eq!(both.samples[201], 3.0 / 32768.0);
        let playable = packet.playable();
        let mut raw = Vec::new();
        for s in &playable {
            raw.extend_from_slice(&s.to_le_bytes());
        }
        assert!(gqvoice::GqVoice::open_bytes(&raw).is_err());
        assert_ne!(packet.as_bytes(), raw.as_slice());
        assert!(!packet.as_bytes().starts_with(b"RIFF"));
        assert!(!dir.join(".samantha.gqvoice.tmp").exists());
        let again = stack.speak("Go.").unwrap();
        let second = gqvoice::GqVoice::open(&path).unwrap();
        assert_eq!(again.samples, second.to_audio().samples);
        assert_eq!(
            second.playable().len(),
            201,
            "a new utterance replaces the file"
        );
        assert_ne!(second.as_bytes(), packet.as_bytes());

        let jdir = dir.join("jarvis");
        std::fs::create_dir_all(&jdir).unwrap();
        let jworker =
            clone_worker::CloneWorker::spawn(&script, clone_worker::CloneModel::Turbo, "jarvis")
                .unwrap();
        let mut jarvis =
            SpeechStack::from_clone(clone_worker::CloneTts::for_test(jworker, jdir.clone()));
        let tone = jarvis.speak("Systems ready.").unwrap();
        let jpath = jdir.join("jarvis.gqvoice");
        let jpacket = gqvoice::GqVoice::open(&jpath).unwrap();
        assert_eq!(jpacket.persona_name(), "jarvis");
        assert_eq!(jpacket.model_name(), "turbo");
        assert_eq!(tone.samples, jpacket.to_audio().samples);
        assert!(!jdir.join("samantha.gqvoice").exists());
        drop(stack);
        drop(jarvis);
        let _ = std::fs::remove_dir_all(&dir);
    }

    const FAKE_CLONE: &str = r#"#!/usr/bin/env python3
import json, struct, sys
while True:
    line = sys.stdin.buffer.readline()
    if not line:
        break
    msg = json.loads(line.decode())
    text = msg.get("text", "")
    n = max(1, min(len(text), 32767))
    pcm = struct.pack("<h", n) + struct.pack("<200h", *([1000 + (n % 50)] * 200))
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("galion-clone-engine: protocol-stub\n")
    sys.stderr.write("galion-clone-utt: %d\n" % len(pcm))
    sys.stderr.flush()
"#;
}
