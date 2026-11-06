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
//!    the pinned safetensors, a local tokenizer, and a sha-allowlisted reference
//!    are on disk. Nano primary, Turbo via `GALION_CLONE_MODEL`. The ref gate is
//!    `GALION_CLONE_LOCAL_REFS=1` plus `$GALION_CLIP_STORE/<persona>/reference.wav`
//!    matching its `.sha256` sidecar, and the consent gate (`galion-consent`)
//!    passing for that voice. Consent runs once, when the slot is built (clone
//!    time); turns read the stored verdict. It does not flip `LOCKS.toml`. Absent
//!    weights or refs skip this slot (no download). `GALION_CLONE_STUB_PCM=1`
//!    does not open this slot. HARD-PASS on the CPU-only loop measures Piper.
//!    A dead child falls back to A21.
//!    A persona whose own ref is missing speaks that utterance with Piper and
//!    leaves the clone child up.
//! 1. [`weights::A21WeightTts`] — the A21 neural TTS slot (FP-03). Activates when a
//!    sha256-verified voice pack is on disk (`GALION_TTS_WEIGHTS` /
//!    `GALION_SPEECH_WEIGHTS` or `<repo>/weights/tts/`) AND a decoder for it exists
//!    ([`weights::decoder_for`]). Today: Piper VITS `en_US-ljspeech-medium` (MIT),
//!    decoded on the CPU by one persistent out-of-process Piper worker
//!    ([`piper_worker`]). With no pack it reports `Absent` and the stack selects
//!    the flite fallback. A clone handoff with that pack absent does the same.
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
//!      pinned pack and an allowlisted ref are on disk, else a21, else flite),
//!      `GALION_CLONE_MODEL=nano|turbo`, `GALION_CLONE_PERSONA=samantha|jarvis`,
//!      `GALION_CLONE_LOCAL_REFS=1`, `GALION_CLIP_STORE`, `GALION_CLONE_WEIGHTS`,
//!      `GALION_CLONE_CKPT`, `GALION_CLONE_TOKENIZER`,
//!      `GALION_A21_RUNTIME` (Piper exe), `GALION_A21_NOISE=natural` (sampled prosody),
//!      `GALION_FLITE_VOICE=slt|kal16|awb|rms|kal` (default `slt`).
//!
//! Prosody on a speak is [`FrameProsody`] from the latest 80 ms
//! `galion-emotion` decision. Pause and loudness are applied to the PCM.
//! Pitch, range, rate, and warmth are carried to the clone child and logged.
//! No empathy sentence is added.

use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub mod clause;
pub mod clip_store;
pub mod clone_cache;
pub mod clone_worker;
pub mod core_map;
pub mod galion_clip;
pub mod flite;
pub mod gqvoice;
pub mod piper_worker;
pub mod sha256;
pub mod weights;

pub use clause::{ClauseBuf, CLAUSE_WORDS};
pub use weights::A21RingFeed;

/// Output rate of every speech backend (Mimi-class 24 kHz lane bus).
pub const SPEECH_RATE: u32 = crate::stub::SAMPLE_RATE;

/// Numeric prosody from one 80 ms `galion-emotion` decision.
///
/// There is no emotion name on this struct. Identity leaves the PCM alone.
/// [`apply_frame_prosody`] uses `pre_pause_ms` and `energy` only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameProsody {
    pub pitch_st: f32,
    pub pitch_range: f32,
    pub rate: f32,
    pub energy: f32,
    pub warmth: f32,
    pub pre_pause_ms: u16,
}

impl Default for FrameProsody {
    fn default() -> Self {
        Self::identity()
    }
}

impl FrameProsody {
    /// No pause, unity loudness. Pitch and rate are the neutral marks.
    pub fn identity() -> Self {
        Self {
            pitch_st: 0.0,
            pitch_range: 1.0,
            rate: 1.0,
            energy: 1.0,
            warmth: 0.5,
            pre_pause_ms: 0,
        }
    }

    /// Same clamps as `galion_emotion::ProsodyControl::clamped`.
    pub fn clamped(self) -> Self {
        Self {
            pitch_st: finite(self.pitch_st, 0.0).clamp(-4.0, 4.0),
            pitch_range: finite(self.pitch_range, 1.0).clamp(0.5, 1.5),
            rate: finite(self.rate, 1.0).clamp(0.8, 1.2),
            energy: finite(self.energy, 1.0).clamp(0.6, 1.2),
            warmth: finite(self.warmth, 0.5).clamp(0.0, 1.0),
            pre_pause_ms: self.pre_pause_ms.min(400),
        }
    }
}

fn finite(x: f32, fallback: f32) -> f32 {
    if x.is_finite() { x } else { fallback }
}

/// Prepend silence and scale loudness. Pitch and rate are not rewritten here.
pub fn apply_frame_prosody(samples: &mut Vec<f32>, rate: u32, prosody: FrameProsody) {
    let prosody = prosody.clamped();
    let energy = prosody.energy;
    if (energy - 1.0).abs() > 1.0e-4 {
        for s in samples.iter_mut() {
            *s = (*s * energy).clamp(-1.0, 1.0);
        }
    }
    if prosody.pre_pause_ms == 0 || rate == 0 {
        return;
    }
    let n = ((u64::from(prosody.pre_pause_ms) * u64::from(rate)) / 1000) as usize;
    if n == 0 {
        return;
    }
    let mut out = vec![0.0; n];
    out.append(samples);
    *samples = out;
}

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
    /// Piper kept beside a live clone child for a persona whose ref is missing.
    piper_side: Option<weights::A21WeightTts>,
    frame_prosody: FrameProsody,
    spoken_id: Option<&'static str>,
    spoken_label: String,
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

// Staged by the missing-pack unit test so selection can be proven without
// libflite and without a voice pack on disk. Unset in every other test.
#[cfg(test)]
thread_local! {
    static TEST_FLITE: std::cell::RefCell<Option<flite::FliteTts>> = const { std::cell::RefCell::new(None) };
}

/// Existing flite fallback. Tests may stage an in-memory [`flite::FliteTts`];
/// otherwise this is [`flite::FliteTts::open_default`].
fn open_flite() -> Result<flite::FliteTts, VoiceError> {
    #[cfg(test)]
    if let Some(tts) = TEST_FLITE.with(|slot| slot.borrow().clone()) {
        return Ok(tts);
    }
    flite::FliteTts::open_default()
}

impl SpeechStack {
    fn bare(
        backend: SpeechBackend,
        weights: weights::WeightStatus,
        engine: Option<Engine>,
    ) -> Self {
        Self {
            backend, weights, engine, piper_side: None, frame_prosody: FrameProsody::identity(),
            spoken_id: None, spoken_label: String::new(), prewarmed: false, utterances: 0,
            cold_first_pcm_ms: None, warm_first_pcm_ms: None, last_first_pcm_ms: None,
            cancel: None, turn_id: 0,
        }
    }

    /// Detect from env + disk. Never downloads anything.
    /// Does not pre-warm; the phone lane calls [`prewarm`](Self::prewarm) once.
    pub fn detect() -> Self {
        let mode = std::env::var("GALION_SPEECH")
            .unwrap_or_else(|_| "auto".into())
            .to_ascii_lowercase();
        let weights = weights::WeightStatus::discover();
        let for_a21 = weights.clone();
        Self::select(
            &mode,
            weights,
            clone_worker::CloneTts::try_new,
            || weights::A21WeightTts::try_new(&for_a21),
            open_flite,
        )
    }

    /// Choose a backend from an already-known weight status.
    ///
    /// A missing Piper voice pack makes [`weights::A21WeightTts::try_new`] fail.
    /// `auto` and `clone` then open the existing flite fallback. `a21` stays
    /// silent. Nothing here downloads a pack.
    fn select(
        mode: &str,
        weights: weights::WeightStatus,
        clone: impl FnOnce() -> Result<clone_worker::CloneTts, String>,
        a21: impl FnOnce() -> Result<weights::A21WeightTts, String>,
        flite: impl FnOnce() -> Result<flite::FliteTts, VoiceError>,
    ) -> Self {
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
            if let Ok(t) = clone() {
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
            if let Ok(t) = a21() {
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
            match flite() {
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

    /// Missing Piper pack, clone slot closed. Used by the unit test so a host
    /// clone install cannot hide the flite choice. Does not read weight files.
    #[cfg(test)]
    fn for_absent_piper_pack(mode: &str) -> Self {
        let weights = weights::WeightStatus::Absent {
            searched: vec!["unit-test: piper voice pack not on disk".into()],
        };
        let for_a21 = weights.clone();
        Self::select(
            mode,
            weights,
            || Err("clone slot closed in the missing-pack unit test".into()),
            || weights::A21WeightTts::try_new(&for_a21),
            open_flite,
        )
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

    /// Register for the next utterance. Clone only. Piper and flite ignore it.
    pub fn set_persona(&mut self, persona: &str) {
        if let Some(Engine::Clone(t)) = self.engine.as_mut() {
            t.set_persona(persona);
        }
    }

    /// 80 ms prosody for the next utterance. Identity until a host sets it.
    pub fn set_frame_prosody(&mut self, prosody: FrameProsody) {
        let prosody = prosody.clamped();
        self.frame_prosody = prosody;
        if let Some(Engine::Clone(t)) = self.engine.as_mut() {
            t.set_prosody(prosody);
        }
    }

    /// Who produced the last successful utterance. Before that, the selected backend.
    pub fn reported(&self) -> (&'static str, String) {
        match self.spoken_id {
            Some(id) => (id, self.spoken_label.clone()),
            None => (self.backend.id(), self.backend.label()),
        }
    }

    /// Discard one short synthesis so the first caller turn is not the cold start.
    /// No PCM is kept. Flite and `off` do nothing.
    pub fn prewarm(&mut self) {
        if self.prewarmed || !self.streams_clauses() {
            return;
        }
        // lat240: three lines of different lengths at call start. The first
        // pays process spawn + model load (legacy piper ~0.4 s cold; galion-piper-rt
        // ~1.4 s, because it loads ORT 1.31 and runs its own two warm-ups before the
        // first line); the next two take the first-run cost of new input lengths
        // so no caller turn sees either.
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

    /// Clone playback ring, when this stack selected the clone slot.
    /// Take the `Arc` before the first speak. Draining it does not generate.
    /// `Heartbeat::step` does not call this.
    pub fn clone_frames(&self) -> Option<Arc<clone_worker::CloneFrameDrain>> {
        match &self.engine {
            Some(Engine::Clone(t)) => Some(t.frames()),
            _ => None,
        }
    }

    pub fn warmth(&self) -> Warmth {
        let (piper_pid, piper_spawns, clone_pid, clone_spawns) = match &self.engine {
            Some(Engine::A21(t)) => (t.piper_pid(), t.piper_spawns(), None, 0),
            Some(Engine::Clone(t)) => {
                let (pid, spawns) = self.piper_side.as_ref().map(|p| (p.piper_pid(), p.piper_spawns())).unwrap_or((None, 0));
                (pid, spawns, t.clone_pid(), t.clone_spawns())
            }
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
            Engine::Clone(t) => {
                if t.ref_ok() {
                    t.synthesize(text, LanguageId::En)
                } else {
                    Err(VoiceError::Backend("clone ref not allowlisted for persona (no clone-time consent approval)".into()))
                }
            }
        };
        let clone_engine = matches!(engine, Engine::Clone(_));
        let soft_ref = clone_engine && matches!(&result, Err(VoiceError::Backend(m)) if m.contains("not allowlisted"));
        let failed_clone = clone_engine
            && result.is_err()
            && !soft_ref
            && !matches!(result, Err(VoiceError::Cancelled));
        self.engine = Some(engine);
        if soft_ref {
            return self.piper_beside(text);
        }
        if failed_clone {
            return self.fallback_piper(text);
        }
        self.finish_audio(result, clone_engine)
    }

    /// Merge #56 + gqvoice: frame prosody (pause, loudness) is applied to
    /// Piper and flite PCM here. Clone PCM is not rewritten after the child:
    /// the landed `.gqvoice` packet must equal the playback buffer, and the
    /// clone child already receives the prosody numbers (`set_prosody`).
    fn finish_audio(
        &mut self,
        result: Result<Audio, VoiceError>,
        clone_engine: bool,
    ) -> Result<Audio, VoiceError> {
        let mut audio = result?;
        let id = self.backend.id();
        let label = self.backend.label();
        if !clone_engine {
            apply_frame_prosody(&mut audio.samples, audio.sample_rate, self.frame_prosody);
        }
        self.spoken_id = Some(id);
        self.spoken_label = label;
        Ok(audio)
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

    /// One Piper utterance without retiring the clone child.
    /// A missing voice pack speaks this utterance with the existing flite fallback.
    fn piper_beside(&mut self, text: &str) -> Result<Audio, VoiceError> {
        self.clear_clone_pending();
        if self.piper_side.is_none() {
            match weights::A21WeightTts::try_new(&self.weights) {
                Ok(piper) => self.piper_side = Some(piper),
                Err(_) => return self.flite_beside(text),
            }
        }
        let piper = self.piper_side.as_mut().expect("piper side");
        let first = piper.synthesize(text, LanguageId::En);
        let audio = if first.is_ok() || piper.worker_alive() {
            first
        } else {
            match piper.respawn() {
                Ok(()) => piper.synthesize(text, LanguageId::En),
                Err(e) => Err(e),
            }
        }?;
        let label = format!("A21 neural TTS (Piper VITS {}; this utterance; clone ref not allowlisted)", piper.voice());
        let mut audio = audio;
        apply_frame_prosody(&mut audio.samples, audio.sample_rate, self.frame_prosody);
        self.spoken_id = Some("a21-weights");
        self.spoken_label = label;
        Ok(audio)
    }

    /// Piper speaks when the clone child cannot. A missing Piper voice pack
    /// selects the existing flite fallback. The lane is not restarted.
    fn fallback_piper(&mut self, text: &str) -> Result<Audio, VoiceError> {
        self.clear_clone_pending();
        if let Ok(piper) = weights::A21WeightTts::try_new(&self.weights) {
            let voice = piper.voice().to_string();
            self.backend = SpeechBackend::A21Weights { voice };
            self.engine = Some(Engine::A21(piper));
            return self.synth_one(text);
        }
        self.fallback_flite(text, "clone worker failed and the Piper voice pack is absent")
    }

    /// This utterance uses flite. The clone child stays up.
    fn flite_beside(&mut self, text: &str) -> Result<Audio, VoiceError> {
        let mut t = open_flite().map_err(|e| {
            VoiceError::Backend(format!(
                "clone ref not allowlisted, Piper voice pack absent; {e}"
            ))
        })?;
        let voice = t.voice().to_string();
        let mut audio = t.synthesize(text, LanguageId::En)?;
        apply_frame_prosody(&mut audio.samples, audio.sample_rate, self.frame_prosody);
        self.spoken_id = Some("system-flite");
        self.spoken_label = SpeechBackend::SystemFlite { voice }.label();
        Ok(audio)
    }

    /// Replace a dead clone slot with the existing flite fallback.
    fn fallback_flite(&mut self, text: &str, why: &str) -> Result<Audio, VoiceError> {
        let t = open_flite().map_err(|e| {
            VoiceError::Backend(format!("{why}; {e}"))
        })?;
        let voice = t.voice().to_string();
        self.backend = SpeechBackend::SystemFlite { voice };
        self.engine = Some(Engine::Flite(t));
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
    fn frame_prosody_identity_is_a_noop_and_pause_is_silence() {
        let mut samples = vec![0.5, -0.25];
        apply_frame_prosody(&mut samples, 24_000, FrameProsody::identity());
        assert_eq!(samples, vec![0.5, -0.25]);
        let mut quiet = vec![1.0];
        apply_frame_prosody(&mut quiet, 24_000, FrameProsody { energy: 0.8, pre_pause_ms: 80, ..FrameProsody::identity() });
        assert_eq!(quiet.len(), 1 + 24_000 * 80 / 1000);
        assert!(quiet[..1920].iter().all(|s| *s == 0.0));
        assert!((quiet[1920] - 0.8).abs() < 1.0e-5);
    }

    #[test] fn resample_len_and_identity() {
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
        assert!(off.clone_frames().is_none());
    }

    /// Missing Piper pack selects the existing flite fallback.
    /// The pack status is built in memory. No weight file is read and nothing is fetched.
    #[test]
    fn missing_piper_pack_selects_flite_without_downloading() {
        let pcm = vec![0.25, -0.5, 0.125, 0.0];
        let _staged = StagedFlite::arm(pcm.clone());
        let absent = weights::WeightStatus::Absent {
            searched: vec!["unit-test: piper voice pack not on disk".into()],
        };
        let err = match weights::A21WeightTts::try_new(&absent) {
            Err(e) => e,
            Ok(_) => panic!("absent piper pack must not open A21"),
        };
        assert!(err.contains("ABSENT"), "{err}");

        let mut auto = SpeechStack::for_absent_piper_pack("auto");
        assert!(matches!(auto.backend, SpeechBackend::SystemFlite { ref voice } if voice == "slt"), "{}", auto.backend.label());
        assert_eq!(auto.backend.speech_token(), "flite");
        assert_eq!(auto.backend.id(), "system-flite");
        assert!(auto.backend.label().contains("NOT A21"));
        assert!(auto.backend.label().contains("NOT neural"));
        assert_eq!(auto.weights.id(), "absent");
        assert!(auto.weights.summary().contains("unit-test: piper voice pack not on disk"));
        assert!(auto.can_speak());
        let audio = auto.speak("Hello from the flite fallback.").unwrap();
        assert_eq!(audio.sample_rate, SPEECH_RATE);
        assert_eq!(audio.samples, pcm);
        assert_eq!(auto.reported().0, "system-flite");

        let clone_mode = SpeechStack::for_absent_piper_pack("clone");
        assert_eq!(clone_mode.backend.speech_token(), "flite");
        assert!(matches!(clone_mode.backend, SpeechBackend::SystemFlite { ref voice } if voice == "slt"));

        let strict = SpeechStack::for_absent_piper_pack("a21");
        assert_eq!(strict.backend.speech_token(), "ABSENT");
        assert!(!strict.can_speak());
        assert!(strict.backend.label().contains("GALION_SPEECH=a21"), "{}", strict.backend.label());

        let off = SpeechStack::for_absent_piper_pack("off");
        assert!(!off.can_speak());
        assert_eq!(off.backend.speech_token(), "ABSENT");
    }

    /// A refused clone ref keeps the child and speaks with flite when the Piper pack is absent.
    #[test]
    fn refused_clone_with_missing_piper_pack_speaks_flite() {
        let pcm = vec![0.25, -0.5, 0.125, 0.0];
        let _staged = StagedFlite::arm(pcm.clone());
        let dir = std::env::temp_dir().join(format!("galion-flite-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, "import sys\nsys.exit(0)\n").unwrap();
        let worker = clone_worker::CloneWorker::spawn_for_test(
            &script,
            clone_worker::CloneModel::Nano,
            "samantha",
        );
        let slot = clone_worker::CloneTts::for_test(worker, dir.clone()).with_verdicts([false, false]);
        let mut stack = SpeechStack::from_clone(slot);
        assert_eq!(stack.weights.id(), "absent");
        let audio = stack.speak("Hello.").unwrap();
        assert_eq!(audio.samples, pcm);
        assert_eq!(stack.reported().0, "system-flite");
        assert!(stack.reported().1.contains("NOT A21"));
        assert!(matches!(stack.backend, SpeechBackend::Clone { .. }), "the clone child stays up");
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            !name.ends_with(".onnx") && !name.ends_with(".wav") && !name.ends_with(".onnx.json")
        }));
        drop(stack);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct StagedFlite;
    impl StagedFlite {
        fn arm(samples: Vec<f32>) -> Self {
            TEST_FLITE.with(|slot| {
                *slot.borrow_mut() = Some(flite::FliteTts::scripted("slt", samples));
            });
            StagedFlite
        }
    }
    impl Drop for StagedFlite {
        fn drop(&mut self) {
            TEST_FLITE.with(|slot| *slot.borrow_mut() = None);
        }
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

    /// Owner decision 2026-11-04: consent runs when the slot is built
    /// (clone time). A turn reads the stored verdict: no consent evaluation,
    /// no reference re-hash, no extra process.
    #[test]
    fn clone_turns_read_the_clone_time_verdict_only() {
        let dir = std::env::temp_dir().join(format!("galion-consent-turn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, FAKE_CLONE).unwrap();
        let worker =
            clone_worker::CloneWorker::spawn(&script, clone_worker::CloneModel::Nano, "samantha")
                .unwrap();
        let mut t = clone_worker::CloneTts::for_test(worker, dir.clone()).with_verdicts([true, false]);
        let before = galion_consent::evaluations();
        for _ in 0..100 {
            assert!(t.ref_ok());
        }
        t.set_persona("jarvis");
        assert!(!t.ref_ok(), "jarvis was refused at clone time");
        t.set_persona("samantha");
        let mut stack = SpeechStack::from_clone(t);
        let spoke = stack.speak("Hello.").unwrap();
        assert!(!spoke.samples.is_empty());
        assert_eq!(galion_consent::evaluations(), before, "a turn evaluated consent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Owner rule: a CC0 voice is selectable only after the CC0 licence was
    /// registered at clone time. Not registered = refused (Piper speaks).
    #[test]
    fn cc0_clone_voice_needs_the_session_cc0_acceptance() {
        let dir = std::env::temp_dir().join(format!("galion-cc0-turn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, FAKE_CLONE).unwrap();
        let worker =
            clone_worker::CloneWorker::spawn(&script, clone_worker::CloneModel::Nano, "samantha")
                .unwrap();
        let t = clone_worker::CloneTts::for_test(worker, dir.clone())
            .with_verdicts([true, true])
            .with_cc0([true, false]);
        let store = galion_consent::cc0_acceptances();
        store.clear();
        assert!(!t.ref_ok(), "not registered: CC0 samantha refused");
        assert!(t.voice_approved("jarvis"), "a non-CC0 voice needs no box");
        store.accept("cc0-test-session");
        assert!(t.ref_ok(), "registered: CC0 samantha allowed");
        store.revoke("cc0-test-session");
        assert!(!t.ref_ok(), "withdrawn: refused");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn landed_clone_is_compiled_before_the_stack_plays_it() {
        let dir = std::env::temp_dir().join(format!("galion-gqvoice-stack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, FAKE_CLONE).unwrap();
        let worker =
            clone_worker::CloneWorker::spawn_for_test(&script, clone_worker::CloneModel::Nano, "samantha");
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
            clone_worker::CloneWorker::spawn_for_test(&script, clone_worker::CloneModel::Turbo, "jarvis");
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
