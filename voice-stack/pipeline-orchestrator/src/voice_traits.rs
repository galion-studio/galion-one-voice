// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Stage traits for the configured persona's conversation loop, plus fakes.
//!
//! Other sectors implement these. This sector runs them. The shapes follow
//! the code already in the repo:
//!
//! * [`Vad`] — `galion-v2-live` `Vad::push` (energy, hangover, start / end).
//! * [`Asr`] — final text after end of turn. The live worker is
//!   `galion-asr-vosk` (NDJSON, 16 kHz, 20 ms frames). A blank final is not
//!   a transcript.
//! * [`ReplyEngine`] — `horbm::Brain::turn`. This crate does not retrieve.
//! * [`Tts`] — one chunk to PCM. The live mouth is Piper
//!   `synthesize_stream`, then the chatterbox worker when a reference is
//!   allowed. Emotion is the [`Pad`] passed in, not words added to the text.
//! * [`AudioIn`] / [`AudioOut`] — mic and speaker. Capture is 16 kHz.
//!   Playback is 24 kHz.
//!
//! [`VOICE_REF_ENV`] is a filesystem path. The old variable is read only as
//! a fallback in the alias helper below. Nothing in this module reads the
//! bytes into the tree.

use crate::clock::{CAPTURE_FRAME_MS, CAPTURE_FRAME_SAMPLES, CAPTURE_RATE_HZ};
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

/// Env var for the configured persona's reference clip.
///
/// A TTS adapter loads it at runtime. This module does not stat the path.
pub const VOICE_REF_ENV: &str = "GALION_VOICE_REF";

/// One resolved reference path, and at most one alias warning.
///
/// `warning` is a single line. It names [`VOICE_REF_ENV`] when the old
/// variable was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceRefChoice<'a> {
    pub value: Option<&'a str>,
    pub warning: Option<&'static str>,
}

/// [`VOICE_REF_ENV`] wins. The old variable is read only when the new
/// variable is unset. When both are set and differ, the new one wins and
/// `warning` is one line.
const LEGACY_VOICE_REF_ENV: &str = "GALION_SAMANTHA_REF";

const FALLBACK_WARNING: &str = "GALION_SAMANTHA_REF is a fallback; set GALION_VOICE_REF";

const DIFFER_WARNING: &str = "GALION_VOICE_REF wins over GALION_SAMANTHA_REF";

/// Pick the reference path from the new variable and the old one.
///
/// Empty strings count as unset. This function does not read the process
/// environment and does not touch the filesystem.
pub fn voice_ref_choice<'a>(
    new_name: Option<&'a str>,
    old_name: Option<&'a str>,
) -> VoiceRefChoice<'a> {
    let new_name = nonempty(new_name);
    let old_name = nonempty(old_name);
    let (value, warning) = match (new_name, old_name) {
        (None, None) => (None, None),
        (Some(value), None) => (Some(value), None),
        (None, Some(value)) => (Some(value), Some(FALLBACK_WARNING)),
        (Some(value), Some(old)) if value == old => (Some(value), None),
        (Some(value), Some(_)) => (Some(value), Some(DIFFER_WARNING)),
    };
    VoiceRefChoice { value, warning }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    match value {
        Some(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}

/// Path named by the environment. Does not stat the path and does not open it.
///
/// A stat here ran on the host turn and missed the socket deadline. The mouth
/// still checks that the path is a file. [`VOICE_REF_ENV`] wins. One fallback
/// warning is printed on this thread, once, with no spawned thread and no
/// cache lock.
pub fn voice_ref_from_env() -> Option<PathBuf> {
    let new_raw = nonempty_os(VOICE_REF_ENV);
    let old_raw = nonempty_os(LEGACY_VOICE_REF_ENV);
    let (chosen, warning) = match (&new_raw, &old_raw) {
        (None, None) => return None,
        (Some(value), None) => (value, None),
        (None, Some(value)) => (value, Some(FALLBACK_WARNING)),
        (Some(value), Some(old)) if value == old => (value, None),
        (Some(value), Some(_)) => (value, Some(DIFFER_WARNING)),
    };
    if let Some(line) = warning {
        warn_line(line);
    }
    Some(PathBuf::from(chosen))
}

fn warn_line(line: &'static str) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if WARNED.swap(true, Ordering::Relaxed) {
        return;
    }
    eprintln!("{line}");
}

fn nonempty_os(key: &str) -> Option<std::ffi::OsString> {
    let raw = std::env::var_os(key)?;
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

/// One capture quantum. Default size is the Vosk frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioFrame {
    pub pcm: Vec<i16>,
    pub rate_hz: u32,
    pub ms: u32,
}

impl AudioFrame {
    /// A mono frame at the capture rate. `amp` is the constant sample.
    pub fn capture(amp: i16) -> Self {
        Self {
            pcm: vec![amp; CAPTURE_FRAME_SAMPLES],
            rate_hz: CAPTURE_RATE_HZ,
            ms: CAPTURE_FRAME_MS,
        }
    }
}

/// PAD mood. Field order matches `horbm::Pad`.
///
/// `RESTING` is the crate constant `{ arousal: 0.3, valence: 0.25, dominance: 0.5 }`.
/// This sector does not invent how the pad moves. [`HoldMood`] keeps the last
/// value. The real brain replaces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Self = Self {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };
}

impl Default for Pad {
    fn default() -> Self {
        Self::RESTING
    }
}

/// Mirror of `horbm::TurnOut`. The real type is not linked.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
}

/// What the speaker and the dot receive after a turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub mood: Pad,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
}

impl Reply {
    pub fn from_turn(turn: TurnOut) -> Self {
        Self {
            text: turn.reply,
            mood: turn.mood,
            intent: turn.intent,
            arm: turn.arm,
            propensity: turn.propensity,
            evidence: turn.evidence,
            sources: turn.sources,
            violations: turn.violations,
            lookup_us: turn.lookup_us,
            took_us: turn.took_us,
        }
    }
}

/// Arguments of `horbm::Brain::open(memory_db, knowledge_dir, cases)`.
///
/// `horbm-memory` pin: `galion-studio/galion-knowledge` rev
/// `761a7f225304fc010379219e5b88c6669dd48f40`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainOpen {
    pub memory_db: PathBuf,
    pub knowledge_dir: PathBuf,
    pub cases: PathBuf,
}

impl BrainOpen {
    pub fn new(
        memory_db: impl Into<PathBuf>,
        knowledge_dir: impl Into<PathBuf>,
        cases: impl Into<PathBuf>,
    ) -> Self {
        Self {
            memory_db: memory_db.into(),
            knowledge_dir: knowledge_dir.into(),
            cases: cases.into(),
        }
    }

    /// The real call stays in crate `horbm`. This sector does not open a
    /// memory file and does not answer from cards.
    pub fn open_unavailable(&self) -> Result<(), &'static str> {
        Err("horbm is not linked; Brain::open stays in crate horbm")
    }
}

/// What `horbm::Brain` already does. Implement this when the crate is present.
pub trait HorbmBrain: Send + 'static {
    fn turn(&mut self, heard: &str) -> TurnOut;
}

/// Adapter from [`HorbmBrain`] to [`ReplyEngine`].
///
/// `Brain::turn` takes the heard text only. The pad on the way out is
/// `TurnOut.mood`. The 80 ms tick keeps publishing between turns.
pub struct HorbmReply<B> {
    pub brain: B,
}

impl<B> HorbmReply<B> {
    pub fn new(brain: B) -> Self {
        Self { brain }
    }
}

impl<B: HorbmBrain> ReplyEngine for HorbmReply<B> {
    fn turn(&mut self, heard: &str, _tick: Pad) -> Reply {
        Reply::from_turn(self.brain.turn(heard))
    }
}

/// Advances once per 80 ms heartbeat. The pipeline calls this off the audio tasks.
pub trait MoodTick: Send + 'static {
    fn advance(&mut self, frame_ms: u32) -> Pad;
    fn set(&mut self, pad: Pad);
    fn get(&self) -> Pad;
}

/// Holds the last pad. No lerp, no scripted curve.
#[derive(Clone, Debug, PartialEq)]
pub struct HoldMood {
    pad: Pad,
}

impl HoldMood {
    pub fn new(pad: Pad) -> Self {
        Self { pad }
    }
}

impl Default for HoldMood {
    fn default() -> Self {
        Self { pad: Pad::RESTING }
    }
}

impl MoodTick for HoldMood {
    fn advance(&mut self, _frame_ms: u32) -> Pad {
        self.pad
    }

    fn set(&mut self, pad: Pad) {
        self.pad = pad;
    }

    fn get(&self) -> Pad {
        self.pad
    }
}

/// Mic. `None` means the device closed.
pub trait AudioIn: Send + 'static {
    fn recv(&mut self) -> impl Future<Output = Option<AudioFrame>> + Send;
}

/// Endpointer event. Same three outcomes as `galion-v2-live` `VadEv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VadEvent {
    None,
    Start,
    End,
}

pub trait Vad: Send + 'static {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent;
    /// Drop the current user turn without emitting [`VadEvent::End`].
    fn reset_turn(&mut self) {}
}

/// Final text. `source` is `fake` here and `vosk` on the live worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub source: &'static str,
    pub took_us: u32,
}

pub trait Asr: Send + 'static {
    fn reset(&mut self);
    fn push(&mut self, frame: &AudioFrame);
    fn finish(&mut self) -> Transcript;
}

/// The reply stage. A fake stands in until `horbm::Brain` is linked.
pub trait ReplyEngine: Send + 'static {
    fn turn(&mut self, heard: &str, tick: Pad) -> Reply;
}

/// One chunk to PCM. Poll `cancel` between chunks and return when it is set.
/// Do not hold the speaker lock across a wait.
pub trait Tts: Send + 'static {
    fn speak(
        &mut self,
        text: &str,
        mood: &Pad,
        cancel: &AtomicBool,
        on_pcm: &mut dyn FnMut(&[i16]),
    );
}

/// Speaker. [`AudioOut::stop`] drops queued PCM and returns. Same name as
/// `galion-sam-contract` `AudioOut::stop`.
pub trait AudioOut: Send + 'static {
    fn write(&mut self, pcm: &[i16]);
    fn stop(&mut self);
}

/// RMS in dB, same formula as `galion-v2-live` `audio::rms_db`.
pub fn rms_db(frame: &[i16]) -> f64 {
    if frame.is_empty() {
        return -120.0;
    }
    let energy = frame
        .iter()
        .map(|&s| (s as f64 / 32768.0).powi(2))
        .sum::<f64>()
        / frame.len() as f64;
    10.0 * energy.max(1e-12).log10()
}

/// Energy VAD. Thresholds match `galion-v2-live` `Vad`.
#[derive(Debug)]
pub struct EnergyVad {
    pub floor_db: f64,
    pub on_db: f64,
    pub speaking: bool,
    pub silent_ms: f64,
    voiced_ms: f64,
    pub hang_ms: f64,
    pub min_voiced_ms: f64,
    pub last_voiced: bool,
}

impl EnergyVad {
    pub fn new(hang_ms: f64) -> Self {
        Self {
            floor_db: -60.0,
            on_db: 12.0,
            speaking: false,
            silent_ms: 0.0,
            voiced_ms: 0.0,
            hang_ms,
            min_voiced_ms: 60.0,
            last_voiced: false,
        }
    }
}

impl Vad for EnergyVad {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent {
        let db = rms_db(&frame.pcm);
        let voiced = db > (self.floor_db + self.on_db).max(-48.0);
        self.last_voiced = voiced;
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        let frame_ms = frame.ms as f64;
        if voiced {
            self.voiced_ms += frame_ms;
            self.silent_ms = 0.0;
        } else {
            self.silent_ms += frame_ms;
        }
        if !self.speaking && voiced && self.voiced_ms >= self.min_voiced_ms {
            self.speaking = true;
            return VadEvent::Start;
        }
        if !voiced && !self.speaking {
            self.voiced_ms = 0.0;
        }
        if self.speaking && self.silent_ms >= self.hang_ms {
            self.speaking = false;
            self.voiced_ms = 0.0;
            return VadEvent::End;
        }
        VadEvent::None
    }

    fn reset_turn(&mut self) {
        self.speaking = false;
        self.voiced_ms = 0.0;
        self.silent_ms = 0.0;
    }
}

/// Bounded mic the tests and the demo push into.
pub struct ChannelIn {
    rx: mpsc::Receiver<AudioFrame>,
}

pub fn mic_channel(bound: usize) -> (mpsc::Sender<AudioFrame>, ChannelIn) {
    let (tx, rx) = mpsc::channel(bound);
    (tx, ChannelIn { rx })
}

impl AudioIn for ChannelIn {
    async fn recv(&mut self) -> Option<AudioFrame> {
        self.rx.recv().await
    }
}

/// Test double. Returns `text` after at least one pushed frame. Empty audio
/// finishes as an empty transcript, so the loop does not invent words.
#[derive(Clone, Debug)]
pub struct FakeAsr {
    pub text: String,
    frames: usize,
}

impl FakeAsr {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            frames: 0,
        }
    }
}

impl Asr for FakeAsr {
    fn reset(&mut self) {
        self.frames = 0;
    }

    fn push(&mut self, _frame: &AudioFrame) {
        self.frames += 1;
    }

    fn finish(&mut self) -> Transcript {
        let text = if self.frames == 0 {
            String::new()
        } else {
            self.text.clone()
        };
        Transcript {
            text,
            source: "fake",
            took_us: 0,
        }
    }
}

/// Test gate. The stage waits until `release` or, for TTS, until cancel.
#[derive(Debug)]
pub struct WaitGate {
    pub release: AtomicBool,
}

impl WaitGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            release: AtomicBool::new(false),
        })
    }
}

/// Test reply. The line is the test's text, not a persona script.
/// Mood is the pad the test set. Words are not rewritten to sound emotional.
pub struct FakeReply {
    pub text: String,
    pub mood: Pad,
    pub calls: Arc<AtomicU32>,
    gate: Option<Arc<WaitGate>>,
}

impl FakeReply {
    pub fn new(text: impl Into<String>, mood: Pad) -> Self {
        Self {
            text: text.into(),
            mood,
            calls: Arc::new(AtomicU32::new(0)),
            gate: None,
        }
    }

    pub fn with_gate(mut self, gate: Arc<WaitGate>) -> Self {
        self.gate = Some(gate);
        self
    }
}

impl ReplyEngine for FakeReply {
    fn turn(&mut self, _heard: &str, _tick: Pad) -> Reply {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            while !gate.release.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
        }
        Reply {
            text: self.text.clone(),
            mood: self.mood,
            intent: String::new(),
            arm: String::new(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: vec!["fake".into()],
            violations: 0,
            lookup_us: 0,
            took_us: 0,
        }
    }
}

/// Test mouth. Writes `first`, then optionally waits, then writes `rest`.
pub struct FakeTts {
    pub first: Vec<i16>,
    pub rest: Vec<i16>,
    pub texts: Arc<Mutex<Vec<String>>>,
    pub moods: Arc<Mutex<Vec<Pad>>>,
    gate: Option<Arc<WaitGate>>,
}

impl FakeTts {
    pub fn pcm(pcm: Vec<i16>) -> Self {
        Self {
            first: pcm,
            rest: Vec::new(),
            texts: Arc::new(Mutex::new(Vec::new())),
            moods: Arc::new(Mutex::new(Vec::new())),
            gate: None,
        }
    }

    /// First chunk is audible, then the call waits so a barge-in can land.
    pub fn gated(first: Vec<i16>, gate: Arc<WaitGate>) -> Self {
        Self {
            first,
            rest: vec![9, 9, 9, 9],
            texts: Arc::new(Mutex::new(Vec::new())),
            moods: Arc::new(Mutex::new(Vec::new())),
            gate: Some(gate),
        }
    }
}

impl Tts for FakeTts {
    fn speak(
        &mut self,
        text: &str,
        mood: &Pad,
        cancel: &AtomicBool,
        on_pcm: &mut dyn FnMut(&[i16]),
    ) {
        self.texts.lock().expect("tts texts").push(text.to_string());
        self.moods.lock().expect("tts moods").push(*mood);
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        if !self.first.is_empty() {
            on_pcm(&self.first);
        }
        if let Some(gate) = &self.gate {
            while !gate.release.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
        }
        if cancel.load(Ordering::SeqCst) || self.rest.is_empty() {
            return;
        }
        on_pcm(&self.rest);
    }
}

#[derive(Debug, Default)]
struct Mem {
    played: Vec<i16>,
    queued: Vec<i16>,
    cancels: u32,
}

/// In-memory speaker. `queued` is what cancel drops. `played` is the history.
#[derive(Clone, Debug, Default)]
pub struct MemorySpeaker {
    inner: Arc<Mutex<Mem>>,
}

impl MemorySpeaker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn played_len(&self) -> usize {
        self.inner.lock().expect("speaker").played.len()
    }

    pub fn queued_len(&self) -> usize {
        self.inner.lock().expect("speaker").queued.len()
    }

    pub fn cancels(&self) -> u32 {
        self.inner.lock().expect("speaker").cancels
    }

    pub fn played(&self) -> Vec<i16> {
        self.inner.lock().expect("speaker").played.clone()
    }
}

impl AudioOut for MemorySpeaker {
    fn write(&mut self, pcm: &[i16]) {
        let mut g = self.inner.lock().expect("speaker");
        g.played.extend_from_slice(pcm);
        g.queued.extend_from_slice(pcm);
    }

    fn stop(&mut self) {
        let mut g = self.inner.lock().expect("speaker");
        g.queued.clear();
        g.cancels = g.cancels.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubBrain;

    impl HorbmBrain for StubBrain {
        fn turn(&mut self, heard: &str) -> TurnOut {
            TurnOut {
                reply: format!("heard {heard}"),
                intent: "greet".into(),
                arm: "none".into(),
                propensity: 0.2,
                evidence: vec!["card".into()],
                sources: vec!["stub-brain".into()],
                violations: 0,
                lookup_us: 10,
                took_us: 20,
                mood: Pad {
                    arousal: 0.4,
                    valence: 0.5,
                    dominance: 0.6,
                },
            }
        }
    }

    #[test]
    fn resting_pad_matches_horbm_constant() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
    }

    #[test]
    fn adapter_maps_turn_out_and_does_not_open_horbm() {
        let open = BrainOpen::new("memory", "knowledge", "cases");
        assert!(open.open_unavailable().is_err());
        let mut engine = HorbmReply::new(StubBrain);
        let reply = engine.turn("hi", Pad::RESTING);
        assert_eq!(reply.text, "heard hi");
        assert_eq!(reply.sources, vec!["stub-brain".to_string()]);
        assert_eq!(reply.mood.arousal, 0.4);
        assert_eq!(reply.lookup_us, 10);
    }

    #[test]
    fn energy_vad_starts_at_60ms_and_ends_after_hang() {
        let mut vad = EnergyVad::new(160.0);
        assert_eq!(vad.push(&AudioFrame::capture(12_000)), VadEvent::None);
        assert_eq!(vad.push(&AudioFrame::capture(12_000)), VadEvent::None);
        assert_eq!(vad.push(&AudioFrame::capture(12_000)), VadEvent::Start);
        for _ in 0..7 {
            assert_eq!(vad.push(&AudioFrame::capture(0)), VadEvent::None);
        }
        assert_eq!(vad.push(&AudioFrame::capture(0)), VadEvent::End);
    }

    #[test]
    fn silence_never_starts() {
        let mut vad = EnergyVad::new(160.0);
        for _ in 0..12 {
            assert_eq!(vad.push(&AudioFrame::capture(0)), VadEvent::None);
        }
        assert!(!vad.speaking);
    }

    #[test]
    fn hold_mood_does_not_drift() {
        let mut mood = HoldMood::default();
        assert_eq!(mood.advance(80), Pad::RESTING);
        let next = Pad {
            arousal: 0.9,
            valence: 0.1,
            dominance: 0.4,
        };
        mood.set(next);
        assert_eq!(mood.advance(80), next);
    }

    #[test]
    fn reference_env_name_is_the_runtime_path() {
        assert_eq!(VOICE_REF_ENV, "GALION_VOICE_REF");
        assert_eq!(LEGACY_VOICE_REF_ENV, "GALION_SAMANTHA_REF");
        let chosen = voice_ref_choice(Some("new"), Some("old"));
        assert_eq!(chosen.value, Some("new"));
        assert_eq!(chosen.warning, Some(DIFFER_WARNING));
    }
}
