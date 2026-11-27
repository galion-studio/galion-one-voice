// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mic to ear to [`horbm_voice::VoiceLoop`] to a mouth adapter.
//!
//! Silero runs in `vad-ear` (its own workspace, so `ort` stays out of this
//! one). The energy gate is [`horbm_voice::EnergyGate`], the same floor and
//! 60 ms onset as the orchestrator. With the ear off, a quiet gap of 640 ms
//! ends one turn. With the ear on, the bundled end-of-turn scorer can end a
//! yield at its mild pause, or hold until its hold pause, and never later
//! than 640 ms plus 80 ms. Vosk stays on a worker thread
//! because its handles are not `Send`. The spoken turn is one
//! [`horbm_voice::VoiceLoop::tick`]: ear, then `turn_heard` (the mic's
//! listener point, or `None` for typed text), then the mouth selected at
//! boot. CPU is the default device. Piper is linked only
//! with `--features piper`. The s711 clone is selected only for an explicit
//! cuda opt-in. The brain is [`crate::brain::HorbmMind`]. `Brain::open` in the
//! orchestrator stays unlinked. A hosted session opens the person diary only
//! when remember is granted.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use galion_sam_asr::{final_text, open_engine, Asr, EngineOpts};
use galion_sam_orch::Tts as OrchTts;
use horbm_voice::{
    EarEvent, EnergyGate, GateEvent, Mouth as VoiceMouth, MouthPull, SpokenLine, TurnBrain,
    VoiceLoop,
};

use crate::audio::{self, rms, SPEAKER_RATE};
use crate::brain::HorbmMind;
use crate::host::HostSession;
use crate::mouth::{self, Mouth, MouthCue};
use crate::native::{self, MindBrain, QueueEar, Stored};
use crate::pipeline::{AudioFeed, ClosedTake, FeedMsg, MicStep, PipeInfo, Pipeline, TurnProduct};
use crate::protocol::{dot_json, error_json, spoken_text_json, turn_json, vad_json};
use crate::reply::{may_speak_piper, Pad, ReplyEngine, TurnOut};

const FRAME: usize = 320;
const SPEECH_RMS: f32 = 0.02;
/// Silence that ends one question when the ear is pause-only. 160 ms split a
/// sentence into two turns. [`HANG_MS`] and [`HANG_MS_WHOLE`] are the same gap.
const HANG_MS: f64 = 640.0;
const HANG_MS_WHOLE: u32 = 640;
/// Latest a sensed take may run past [`HANG_MS`].
const HANG_SLACK_MS: u32 = 80;
/// Capture quantum passed to [`horbm_voice::EnergyGate::push`].
const GATE_FRAME_MS: u32 = 20;
/// The energy gate's clamp (−48 dB). At or under this, the sensed turn clock
/// counts silence. The pause-only path does not read it.
const GATE_CLAMP_DB: f64 = -48.0;
const UTTERANCE_CAP: usize = 16_000 * 16;

pub fn live_factory() -> Arc<dyn Fn() -> Box<dyn crate::pipeline::Pipeline> + Send + Sync> {
    Arc::new(|| Box::new(LivePipeline::from_env()))
}

/// One factory for every socket of a booted host. The mind, seal, and log are shared.
pub fn live_factory_with(
    host: Arc<HostSession>,
) -> Arc<dyn Fn() -> Box<dyn crate::pipeline::Pipeline> + Send + Sync> {
    Arc::new(move || Box::new(LivePipeline::from_host(Arc::clone(&host))))
}

enum Front {
    Child(ChildVad),
    Energy(EnergyGate),
}

struct OpenVad {
    front: Front,
    backend: &'static str,
    ready: bool,
}

struct OpenAsr {
    ear: Ear,
    name: &'static str,
    worker: Option<JoinHandle<()>>,
}

enum Ear {
    Script(String),
    Missing,
    Vosk(Sender<Job>),
}

struct Job {
    pcm: Vec<i16>,
    back: Sender<Result<String, String>>,
}

#[cfg(test)]
struct RecMouth {
    heard: Arc<Mutex<Vec<String>>>,
    cues: Arc<Mutex<Vec<MouthCue>>>,
}

#[cfg(test)]
impl Mouth for RecMouth {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        if let Ok(mut guard) = self.heard.lock() {
            guard.push(text.to_string());
        }
        if let Ok(mut guard) = self.cues.lock() {
            guard.push(*cue);
        }
        Ok((vec![1000, -1000, 1000, -1000], crate::audio::SPEAKER_RATE))
    }
    fn mouth(&self) -> &'static str {
        "sample"
    }
    fn voice(&self) -> &'static str {
        "sample"
    }
    fn clone_status(&self) -> &'static str {
        "off"
    }
}

enum MouthJob {
    Speak {
        text: String,
        cue: MouthCue,
        lead: String,
        rest: String,
        cancel: Arc<AtomicBool>,
        tx: tokio::sync::mpsc::UnboundedSender<FeedMsg>,
    },
    Shutdown,
}

/// One mouth thread. `stop` sets the in-flight job's cancel flag. The thread
/// polls that flag between sentences and inside any wait, then returns to
/// `recv`, so shutdown can join it.
struct SpeakWorker {
    tx: std::sync::mpsc::Sender<MouthJob>,
    cancel: Option<Arc<AtomicBool>>,
    join: Option<JoinHandle<()>>,
    mouth: &'static str,
    voice: &'static str,
    clone_status: &'static str,
}

impl SpeakWorker {
    fn spawn(mut mouth: Box<dyn Mouth>) -> Self {
        let label = mouth.mouth();
        let voice = mouth.voice();
        let clone_status = mouth.clone_status();
        let (tx, rx) = std::sync::mpsc::channel();
        let join = std::thread::spawn(move || worker_loop(&mut mouth, rx));
        Self {
            tx,
            cancel: None,
            join: Some(join),
            mouth: label,
            voice,
            clone_status,
        }
    }

    fn mouth(&self) -> &'static str {
        self.mouth
    }

    fn voice(&self) -> &'static str {
        self.voice
    }

    fn clone_status(&self) -> &'static str {
        self.clone_status
    }

    fn cancel(&mut self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, Ordering::SeqCst);
        }
    }

    fn start(&mut self, text: String, cue: MouthCue, lead: String, rest: String) -> AudioFeed {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(Arc::clone(&cancel));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = self.tx.send(MouthJob::Speak {
            text,
            cue,
            lead,
            rest,
            cancel,
            tx,
        });
        AudioFeed { rx }
    }
}

impl Drop for SpeakWorker {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.tx.send(MouthJob::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn worker_loop(mouth: &mut Box<dyn Mouth>, rx: std::sync::mpsc::Receiver<MouthJob>) {
    while let Ok(job) = rx.recv() {
        let MouthJob::Speak {
            text,
            cue,
            lead,
            rest,
            cancel,
            tx,
        } = job
        else {
            break;
        };
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(FeedMsg::End);
            continue;
        }
        mouth.set_clauses(&lead, &rest);
        if !mouth.emits_audio() {
            let result = mouth.speak_stream(&text, &cue, &cancel, &mut |_| false);
            match result {
                Ok(()) => {
                    for line in mouth.drain_lines() {
                        if cancel.load(Ordering::SeqCst) {
                            break;
                        }
                        if tx
                            .send(FeedMsg::Text(spoken_text_json(&line, cue.mood)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    let _ = tx.send(FeedMsg::End);
                }
                Err(code) => {
                    let _ = tx.send(FeedMsg::Failed(code));
                }
            }
            continue;
        }
        let mut emitted = false;
        let result = mouth.speak_stream(&text, &cue, &cancel, &mut |pcm| {
            if cancel.load(Ordering::SeqCst) || pcm.is_empty() {
                return !cancel.load(Ordering::SeqCst);
            }
            for slice in audio::slice_pcm(pcm, SPEAKER_RATE, crate::audio::FRAME_MS) {
                if cancel.load(Ordering::SeqCst) {
                    return false;
                }
                if tx.send(FeedMsg::Slice(slice)).is_err() {
                    return false;
                }
                emitted = true;
            }
            true
        });
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(FeedMsg::End);
            continue;
        }
        match result {
            Ok(()) if emitted => {
                let _ = tx.send(FeedMsg::End);
            }
            Ok(()) => {
                let _ = tx.send(FeedMsg::Failed("piper-missing".into()));
            }
            Err(code) => {
                let _ = tx.send(FeedMsg::Failed(code));
            }
        }
    }
}

pub struct LivePipeline {
    front: Front,
    ear: Ear,
    mind: Arc<Mutex<HorbmMind>>,
    host: Option<Arc<HostSession>>,
    last: Arc<Mutex<Option<Stored>>>,
    feed: Arc<Mutex<Option<AudioFeed>>>,
    #[cfg_attr(not(test), allow(dead_code))]
    turns: Arc<AtomicU32>,
    voice: VoiceLoop<QueueEar, LoopMouth, ProsodyBrain>,
    worker: Arc<Mutex<SpeakWorker>>,
    mouth_device: &'static str,
    vad_backend: &'static str,
    vad_ready: bool,
    ear_name: &'static str,
    phase: &'static str,
    level: f32,
    speech: Vec<i16>,
    in_speech: bool,
    utterance: Vec<i16>,
    /// Prosody ear. One per pipeline, on its own thread: the pitch crate is
    /// not `Send`, and this pipeline is. Sensing is on unless
    /// `GALION_EAR_SENSE` is `0`, `off`, or `deny`.
    prosody: ProsodyEar,
    /// Leftover samples short of one 1280-sample frame. Not a heap buffer.
    prosody_carry: [i16; horbm_ear::FRAME_SAMPLES],
    prosody_carry_len: usize,
    /// Reused f32 frame. Filled in place. Not allocated per frame.
    prosody_frame: [f32; horbm_ear::FRAME_SAMPLES],
    /// Latest listener point from this utterance. Cleared when the turn runs.
    prosody_point: Option<horbm::Heard>,
    /// Latest [`horbm_ear::TurnMsg`] from the prosody thread.
    prosody_turn: horbm_ear::TurnMsg,
    /// From the bundled `eot.toml`, read once. Not parsed per frame.
    mild_pause_ms: u32,
    hold_pause_ms: u32,
    /// Quiet run after the energy gate opened this take, in milliseconds.
    silence_ms: u32,
    /// The energy gate has opened a take and this socket has not closed it.
    turn_open: bool,
    /// The gate already emitted end and this take is still holding.
    held_past_gate: bool,
    pending_ear: Arc<Mutex<Option<horbm::Heard>>>,
    ear_info_logged: AtomicBool,
    /// Set when a heard line finished. Idle thoughts wait from this instant.
    last_heard: Option<Instant>,
    last_inner: Option<Instant>,
    last_pad: Option<(u32, u32, u32)>,
    _worker: Option<JoinHandle<()>>,
}

struct Opened {
    vad: OpenVad,
    asr: OpenAsr,
}

impl LivePipeline {
    pub fn from_env() -> Self {
        let raw = std::env::var("GALION_MOUTH").unwrap_or_default();
        let (pick, _) = mouth::parse_mouth_env(&raw);
        let choice = mouth::decide(&mouth::read_input());
        let device = match pick {
            mouth::MouthPick::Native => native::BOOT_MOUTH_DEVICE,
            mouth::MouthPick::Today => mouth::mouth_device(&choice),
        };
        Self::assemble(
            Opened {
                vad: open_vad(),
                asr: open_ear(),
            },
            Arc::new(Mutex::new(HorbmMind::from_env())),
            None,
            mouth::open_kind(pick, &choice, &raw),
            device,
            ProsodyBoot::Process,
        )
    }

    pub(crate) fn from_host(host: Arc<HostSession>) -> Self {
        let raw = std::env::var("GALION_MOUTH").unwrap_or_default();
        let (pick, _) = mouth::parse_mouth_env(&raw);
        let choice = mouth::decide(&mouth::read_input());
        let device = match pick {
            mouth::MouthPick::Native => native::BOOT_MOUTH_DEVICE,
            mouth::MouthPick::Today => mouth::mouth_device(&choice),
        };
        Self::assemble(
            Opened {
                vad: open_vad(),
                asr: open_ear(),
            },
            host.mind(),
            Some(host),
            mouth::open_kind(pick, &choice, &raw),
            device,
            ProsodyBoot::Process,
        )
    }

    #[cfg(test)]
    fn for_test(
        front: Front,
        backend: &'static str,
        ready: bool,
        ear: Ear,
        ear_name: &'static str,
        mind: HorbmMind,
        mouth: Box<dyn Mouth>,
    ) -> Self {
        Self::assemble(
            Opened {
                vad: OpenVad {
                    front,
                    backend,
                    ready,
                },
                asr: OpenAsr {
                    ear,
                    name: ear_name,
                    worker: None,
                },
            },
            Arc::new(Mutex::new(mind)),
            None,
            mouth,
            native::BOOT_MOUTH_DEVICE,
            // The 640 ms hang tests lock the gate. Ear-on timing is ear_live.
            ProsodyBoot::ExplicitOff,
        )
    }

    fn assemble(
        opened: Opened,
        mind: Arc<Mutex<HorbmMind>>,
        host: Option<Arc<HostSession>>,
        mouth: Box<dyn Mouth>,
        mouth_device: &'static str,
        prosody: ProsodyBoot,
    ) -> Self {
        let Opened { vad, asr } = opened;
        let prosody = ProsodyEar::spawn(prosody);
        eprintln!("{}", ear_state_line(prosody.sensing()));
        let eot = horbm_ear::EotConfig::bundled();
        let prosody_turn = if prosody.sensing() {
            horbm_ear::TurnMsg::Hold
        } else {
            horbm_ear::TurnMsg::PauseOnly
        };
        let pending_ear = Arc::new(Mutex::new(None));
        let brain = ProsodyBrain {
            inner: MindBrain::shared(Arc::clone(&mind)),
            pending: Arc::clone(&pending_ear),
        };
        let last = Arc::clone(&brain.inner.last);
        let turns = Arc::clone(&brain.inner.turns);
        let feed = Arc::new(Mutex::new(None));
        let worker = Arc::new(Mutex::new(SpeakWorker::spawn(mouth)));
        let voice = VoiceLoop::new(
            QueueEar::default(),
            LoopMouth {
                worker: Arc::clone(&worker),
                feed: Arc::clone(&feed),
                last: Arc::clone(&last),
            },
            brain,
        );
        Self {
            front: vad.front,
            ear: asr.ear,
            mind,
            host,
            last,
            feed,
            turns,
            voice,
            worker,
            mouth_device,
            vad_backend: vad.backend,
            vad_ready: vad.ready,
            ear_name: asr.name,
            phase: "idle",
            level: 0.0,
            speech: Vec::new(),
            in_speech: false,
            utterance: Vec::new(),
            prosody,
            prosody_carry: [0; horbm_ear::FRAME_SAMPLES],
            prosody_carry_len: 0,
            prosody_frame: [0.0; horbm_ear::FRAME_SAMPLES],
            prosody_point: None,
            prosody_turn,
            mild_pause_ms: eot.mild_pause_ms,
            hold_pause_ms: eot.hold_pause_ms,
            silence_ms: 0,
            turn_open: false,
            held_past_gate: false,
            pending_ear,
            ear_info_logged: AtomicBool::new(false),
            last_heard: None,
            last_inner: None,
            last_pad: None,
            _worker: asr.worker,
        }
    }

    #[cfg(test)]
    fn voice_turns(&self) -> u32 {
        self.turns.load(Ordering::Relaxed)
    }

    fn fail(&mut self, code: &str) -> TurnProduct {
        self.phase = "idle";
        TurnProduct::events(vec![error_json(code), dot_json("idle", self.level)])
    }

    fn hear(
        &mut self,
        text: &str,
        ear: Option<horbm::Heard>,
        t_ms: u64,
        preface: Vec<String>,
    ) -> TurnProduct {
        let _ = t_ms;
        *self
            .pending_ear
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = ear;
        self.voice.ear_mut().push(EarEvent::end(text));
        let _frame = self.voice.tick(&[]);
        let stored = self
            .last
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        let feed = self
            .feed
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        let out = match stored {
            Some(Stored::Out(out)) => out,
            Some(Stored::Err(code)) => return self.fail(&code),
            None => return self.fail("brain-unlinked"),
        };
        self.note_turn(text, &out);
        let reply = out.reply.clone();
        let native = self
            .worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .mouth()
            == "native";
        let mut events = preface;
        let Some(feed) = feed else {
            self.phase = "idle";
            events.push(turn_json(&out));
            events.push(dot_json("idle", self.level));
            return TurnProduct {
                events,
                pcm: Vec::new(),
                rate: SPEAKER_RATE,
                reply,
                feed: None,
            };
        };
        self.phase = "speaking";
        if !native {
            events.push(turn_json(&out));
        }
        events.push(dot_json("speaking", self.level));
        TurnProduct {
            events,
            pcm: Vec::new(),
            rate: SPEAKER_RATE,
            reply,
            feed: Some(feed),
        }
    }

    fn note_turn(&mut self, heard: &str, out: &crate::reply::TurnOut) {
        self.last_heard = Some(Instant::now());
        if let Some(host) = &self.host {
            host.on_turn(heard, out);
        }
    }

    fn push_frame(&mut self, frame: &[i16]) -> Option<&'static str> {
        self.level = rms(frame).clamp(0.0, 1.0);
        let event = match &mut self.front {
            Front::Child(child) => {
                return match child.push(frame) {
                    Some("start") => Some("start"),
                    Some("end") => Some("end"),
                    _ => None,
                };
            }
            Front::Energy(vad) => vad.push(frame, 20.0),
        };
        if self.ear_pause_only() {
            if self.held_past_gate && self.turn_open {
                self.close_energy_turn();
                return Some("end");
            }
            return match event {
                GateEvent::Start => Some("start"),
                GateEvent::End => Some("end"),
                GateEvent::None => None,
            };
        }
        self.turn_from_ear(frame, event)
    }

    /// Ear off, denied, or a scorer frame that said pause-only.
    fn ear_pause_only(&self) -> bool {
        !self.prosody.sensing() || matches!(self.prosody_turn, horbm_ear::TurnMsg::PauseOnly)
    }

    /// Energy gate is in silence after speech. Yield ends at the mild pause.
    /// Hold does not end before the hold pause, and never later than the hang
    /// plus [`HANG_SLACK_MS`].
    fn turn_from_ear(&mut self, frame: &[i16], event: GateEvent) -> Option<&'static str> {
        if matches!(event, GateEvent::Start) && !self.held_past_gate {
            self.turn_open = true;
            self.silence_ms = 0;
            return Some("start");
        }
        if !self.turn_open {
            return match event {
                GateEvent::Start => Some("start"),
                GateEvent::End => Some("end"),
                GateEvent::None => None,
            };
        }
        if gate_quiet(frame) {
            self.silence_ms = self.silence_ms.saturating_add(GATE_FRAME_MS);
        } else if !matches!(event, GateEvent::End) {
            self.silence_ms = 0;
        }
        if matches!(event, GateEvent::End) {
            self.silence_ms = self.silence_ms.max(HANG_MS_WHOLE);
            self.held_past_gate = true;
        }
        let cap = HANG_MS_WHOLE.saturating_add(HANG_SLACK_MS);
        let end_now = match self.prosody_turn {
            horbm_ear::TurnMsg::Yield { .. } => self.silence_ms >= self.mild_pause_ms,
            horbm_ear::TurnMsg::Hold => self.silence_ms >= self.hold_pause_ms.min(cap),
            horbm_ear::TurnMsg::PauseOnly => matches!(event, GateEvent::End),
        };
        if end_now {
            self.close_energy_turn();
            Some("end")
        } else {
            None
        }
    }

    fn close_energy_turn(&mut self) {
        self.turn_open = false;
        self.silence_ms = 0;
        self.held_past_gate = false;
        if let Front::Energy(vad) = &mut self.front {
            vad.reset_turn();
        }
    }
}

fn gate_quiet(frame: &[i16]) -> bool {
    // Same formula as the energy gate. `rms_db` is not re-exported from `horbm_voice`.
    galion_sam_orch::voice_traits::rms_db(frame) <= GATE_CLAMP_DB
}

impl Pipeline for LivePipeline {
    fn push_mic(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        match self.step_mic(pcm, rate, t_ms) {
            MicStep::Product(product) => product,
            MicStep::Pending(take) => self.finish_take(take),
        }
    }

    fn step_mic(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> MicStep {
        if rate != 16_000 && rate != 24_000 {
            return MicStep::Product(self.fail("bad-rate"));
        }
        let frame_n = audio::samples_per_frame(rate, 20);
        if frame_n == 0 {
            return MicStep::Product(self.fail("bad-frame"));
        }
        self.speech.extend_from_slice(pcm);
        let mut events = Vec::new();
        let mut buf = std::mem::take(&mut self.speech);
        while buf.len() >= frame_n {
            let frame: Vec<i16> = buf.drain(..frame_n).collect();
            let kind = self.push_frame(&frame);
            let piece = frame_16k(&frame, rate);
            match kind {
                Some("start") => {
                    self.in_speech = true;
                    self.utterance.clear();
                    self.utterance.extend_from_slice(&piece);
                    self.reset_prosody_utterance();
                    self.push_prosody(&piece);
                    self.phase = "listening";
                    events.push(vad_json("start", t_ms));
                    events.push(dot_json("listening", self.level));
                }
                Some("end") => {
                    if self.in_speech {
                        self.utterance.extend_from_slice(&piece);
                        self.push_prosody(&piece);
                    }
                    self.in_speech = false;
                    events.push(vad_json("end", t_ms));
                    let heard_pcm = std::mem::take(&mut self.utterance);
                    self.speech = buf;
                    if heard_pcm.is_empty() {
                        let _ = self.take_ear_point();
                        self.phase = "idle";
                        events.push(dot_json("idle", self.level));
                        return MicStep::Product(TurnProduct::events(events));
                    }
                    // Recognition and the brain run in `finish_take`. The
                    // listener point stays on this pipe until then.
                    return MicStep::Pending(ClosedTake::speech(events, heard_pcm, t_ms));
                }
                _ => {
                    if self.in_speech {
                        self.utterance.extend_from_slice(&piece);
                        self.push_prosody(&piece);
                        if self.utterance.len() > UTTERANCE_CAP {
                            let drop_n = self.utterance.len() - UTTERANCE_CAP;
                            self.utterance.drain(..drop_n);
                        }
                    }
                }
            }
        }
        self.speech = buf;
        MicStep::Product(TurnProduct::events(events))
    }

    fn finish_take(&mut self, take: ClosedTake) -> TurnProduct {
        let ClosedTake {
            mut events,
            t_ms,
            pcm,
            ..
        } = take;
        let point = self.take_ear_point();
        let heard = match self.recognise(&pcm) {
            Ok(text) => text,
            Err(code) => {
                let mut fail = self.fail(&code);
                events.append(&mut fail.events);
                fail.events = events;
                return fail;
            }
        };
        self.hear(&heard, point, t_ms, events)
    }

    fn commit_pcm(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        if !self.vad_ready {
            return self.fail("vad-unready");
        }
        let samples = if rate == 16_000 {
            pcm.to_vec()
        } else if rate == 24_000 {
            downsample_24_to_16(pcm)
        } else {
            return self.fail("bad-rate");
        };
        self.speech.clear();
        self.reset_prosody_utterance();
        self.push_prosody(&samples);
        let mut preface = Vec::new();
        let mut saw_speech = rms(&samples) >= SPEECH_RMS;
        let mut off = 0;
        while off + FRAME <= samples.len() {
            let frame = &samples[off..off + FRAME];
            off += FRAME;
            if let Some(kind) = self.push_frame(frame) {
                preface.push(vad_json(kind, t_ms));
                if kind == "start" {
                    saw_speech = true;
                    self.phase = "listening";
                    preface.push(dot_json("listening", self.level));
                }
            }
        }
        self.speech.clear();
        if !saw_speech {
            let _ = self.take_ear_point();
            self.phase = "idle";
            preface.push(dot_json("idle", self.level));
            return TurnProduct::events(preface);
        }
        let point = self.take_ear_point();
        let heard = match self.recognise(&samples) {
            Ok(text) => text,
            Err(code) => return self.fail(&code),
        };
        self.hear(&heard, point, t_ms, preface)
    }

    fn push_text(&mut self, text: &str, t_ms: u64) -> TurnProduct {
        let text = text.trim();
        if text.is_empty() {
            return self.fail("empty-text");
        }
        let _ = self.take_ear_point();
        self.phase = "thinking";
        self.hear(text, None, t_ms, vec![dot_json("thinking", self.level)])
    }

    fn stop(&mut self) {
        self.worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .cancel();
        self.speech.clear();
        self.utterance.clear();
        self.in_speech = false;
        let _ = self.take_ear_point();
        self.phase = "idle";
        self.turn_open = false;
        self.silence_ms = 0;
        self.held_past_gate = false;
        if let Front::Energy(vad) = &mut self.front {
            vad.reset_turn();
        }
        if let Front::Child(child) = &mut self.front {
            child.reset();
        }
    }

    fn utterance_done(&mut self) {
        if self.phase == "speaking" {
            self.phase = "idle";
        }
    }

    fn tick_mood(&mut self) -> (crate::reply::Pad, &'static str) {
        let (pad, source) = match self
            .mind
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .tick()
        {
            Some(pad) => (pad.finite_or_resting(), "brain"),
            None => (crate::reply::Pad::RESTING, "resting"),
        };
        let key = (
            pad.arousal.to_bits(),
            pad.valence.to_bits(),
            pad.dominance.to_bits(),
        );
        if self.last_pad != Some(key) {
            self.last_pad = Some(key);
            if let Some(host) = &self.host {
                host.on_mood(pad.arousal, pad.valence, pad.dominance);
            }
        }
        (pad, source)
    }

    fn poll_inner(&mut self) -> Option<String> {
        // Quiet after a heard line, on the existing mood tick. Not inside a turn.
        if self.phase != "idle" {
            return None;
        }
        let heard_at = self.last_heard?;
        let quiet = Duration::from_millis(u64::from(horbm::WAIT_MS));
        if heard_at.elapsed() < quiet {
            return None;
        }
        if self.last_inner.is_some_and(|at| at.elapsed() < quiet) {
            return None;
        }
        let inner = self
            .mind
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .idle_inner()
            .ok()?;
        self.last_inner = Some(Instant::now());
        if let Some(host) = &self.host {
            host.on_inner(
                &inner.verb,
                &inner.lean,
                inner.surprise,
                inner.learned,
                &inner.slots,
            );
        }
        Some(crate::protocol::inner_json(
            &inner.verb,
            &inner.lean,
            inner.surprise,
            inner.learned,
            &inner.slots,
        ))
    }

    fn persona_id(&self) -> String {
        self.mind
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .persona_id()
    }

    fn phase(&self) -> &'static str {
        self.phase
    }

    fn level(&self) -> f32 {
        self.level
    }

    fn info(&self) -> PipeInfo {
        if !self.ear_info_logged.swap(true, Ordering::Relaxed) {
            eprintln!("{}", ear_state_line(self.prosody.sensing()));
        }
        let mind = self.mind.lock().unwrap_or_else(|err| err.into_inner());
        let knowledge_cards = mind.knowledge_cards();
        let knowledge_dir = mind.knowledge_path().display().to_string();
        drop(mind);
        let mouth = self.worker.lock().unwrap_or_else(|err| err.into_inner());
        PipeInfo {
            ear: self.ear_name,
            mouth: mouth.mouth(),
            voice: mouth.voice(),
            clone_status: mouth.clone_status(),
            mouth_device: self.mouth_device,
            mind: native::BOOT_BRAIN.into(),
            vad_backend: self.vad_backend,
            vad_ready: self.vad_ready,
            knowledge_cards,
            knowledge_dir,
        }
    }
}

/// Mouth the voice loop starts. Playback stays on the existing worker thread.
struct LoopMouth {
    worker: Arc<Mutex<SpeakWorker>>,
    feed: Arc<Mutex<Option<AudioFeed>>>,
    last: Arc<Mutex<Option<Stored>>>,
}

impl VoiceMouth for LoopMouth {
    fn id(&self) -> &str {
        "host"
    }

    fn start(&mut self, line: &SpokenLine) {
        if line.text.trim().is_empty() {
            return;
        }
        let cue = MouthCue::from_spoken(line);
        let (lead, rest) = match self
            .last
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
        {
            Some(Stored::Out(out)) if out.reply == line.text => {
                (out.first_clause.clone(), out.rest.clone())
            }
            _ => (String::new(), String::new()),
        };
        let feed = self
            .worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .start(line.text.clone(), cue, lead, rest);
        *self.feed.lock().unwrap_or_else(|err| err.into_inner()) = Some(feed);
    }

    fn pull(&mut self) -> MouthPull {
        MouthPull::Idle
    }

    fn cancel(&mut self) {
        self.worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .cancel();
    }

    fn speaking(&self) -> bool {
        false
    }
}

impl LivePipeline {
    fn recognise(&mut self, pcm: &[i16]) -> Result<String, String> {
        match &self.ear {
            Ear::Missing => Err("asr-unready".into()),
            Ear::Script(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    Err("asr-empty".into())
                } else {
                    Ok(line)
                }
            }
            Ear::Vosk(tx) => {
                let (back_tx, back_rx) = mpsc::channel();
                tx.send(Job {
                    pcm: pcm.to_vec(),
                    back: back_tx,
                })
                .map_err(|_| "asr-unready".to_string())?;
                back_rx
                    .recv_timeout(Duration::from_secs(30))
                    .unwrap_or(Err("asr-unready".into()))
            }
        }
    }

    fn reset_prosody_utterance(&mut self) {
        self.prosody_carry_len = 0;
        self.prosody_point = None;
    }

    fn take_ear_point(&mut self) -> Option<horbm::Heard> {
        self.prosody_carry_len = 0;
        self.prosody_point.take()
    }

    /// Push 16 kHz samples. A short tail stays in `prosody_carry` until the
    /// next samples fill one frame. The f32 frame is the array filled below.
    fn push_prosody(&mut self, pcm: &[i16]) {
        let mut off = 0;
        while off < pcm.len() {
            let room = horbm_ear::FRAME_SAMPLES - self.prosody_carry_len;
            let n = room.min(pcm.len() - off);
            self.prosody_carry[self.prosody_carry_len..self.prosody_carry_len + n]
                .copy_from_slice(&pcm[off..off + n]);
            self.prosody_carry_len += n;
            off += n;
            if self.prosody_carry_len == horbm_ear::FRAME_SAMPLES {
                self.flush_prosody_frame();
            }
        }
    }

    fn flush_prosody_frame(&mut self) {
        for (slot, sample) in self.prosody_frame.iter_mut().zip(self.prosody_carry.iter()) {
            // Same scale as `crate::audio::rms`: full-scale i16 is about 1.0.
            *slot = f32::from(*sample) / 32768.0;
        }
        self.prosody_carry_len = 0;
        let (point, turn) = self.prosody.push_frame(&self.prosody_frame);
        self.prosody_turn = turn;
        if let Some(point) = point {
            self.prosody_point = Some(point);
        }
    }
}

/// How the prosody thread builds its ear. The ear is created on that thread:
/// the pitch crate keeps `Rc` scratch, so an `Ear` is not `Send`.
enum ProsodyBoot {
    /// `Ear::from_env` on `GALION_EAR_SENSE`. Unset stays on.
    Process,
    ExplicitOn,
    ExplicitOff,
}

fn boot_ear(boot: ProsodyBoot) -> horbm_ear::Ear {
    match boot {
        ProsodyBoot::Process => horbm_ear::Ear::from_process_env(),
        ProsodyBoot::ExplicitOn => horbm_ear::Ear::from_env(None),
        ProsodyBoot::ExplicitOff => horbm_ear::Ear::from_env(Some("off")),
    }
}

/// The pitch detector keeps `Rc` scratch, so it cannot sit on this `Send`
/// pipeline. One thread owns the ear for the life of the pipeline. The frame
/// array is the only buffer, filled by the caller and copied into the slot.
struct ProsodyEar {
    slot: Arc<(Mutex<ProsodySlot>, Condvar)>,
    sensing: bool,
    join: Option<JoinHandle<()>>,
}

struct ProsodySlot {
    frame: [f32; horbm_ear::FRAME_SAMPLES],
    state: ProsodyState,
    kept: Option<horbm::Heard>,
    turn: horbm_ear::TurnMsg,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProsodyState {
    Idle,
    Push,
    Shutdown,
}

impl ProsodyEar {
    fn spawn(boot: ProsodyBoot) -> Self {
        let slot = Arc::new((
            Mutex::new(ProsodySlot {
                frame: [0.0; horbm_ear::FRAME_SAMPLES],
                state: ProsodyState::Idle,
                kept: None,
                turn: horbm_ear::TurnMsg::PauseOnly,
            }),
            Condvar::new(),
        ));
        let started = Arc::new((Mutex::new(None), Condvar::new()));
        let worker = Arc::clone(&slot);
        let ready = Arc::clone(&started);
        let join = thread::Builder::new()
            .name("galion-prosody-ear".into())
            .spawn(move || {
                let ear = boot_ear(boot);
                {
                    let (lock, wake) = &*ready;
                    *lock.lock().unwrap_or_else(|err| err.into_inner()) = Some(ear.sensing());
                    wake.notify_one();
                }
                prosody_loop(ear, worker);
            })
            .ok();
        let sensing = if join.is_some() {
            let (lock, wake) = &*started;
            let mut guard = lock.lock().unwrap_or_else(|err| err.into_inner());
            while guard.is_none() {
                guard = wake.wait(guard).unwrap_or_else(|err| err.into_inner());
            }
            guard.unwrap_or(false)
        } else {
            false
        };
        Self {
            slot,
            sensing,
            join,
        }
    }

    fn sensing(&self) -> bool {
        self.sensing
    }

    fn push_frame(
        &self,
        frame: &[f32; horbm_ear::FRAME_SAMPLES],
    ) -> (Option<horbm::Heard>, horbm_ear::TurnMsg) {
        let Some(_) = self.join.as_ref() else {
            return (None, horbm_ear::TurnMsg::PauseOnly);
        };
        let (lock, wake) = &*self.slot;
        let mut slot = lock.lock().unwrap_or_else(|err| err.into_inner());
        slot.frame = *frame;
        slot.kept = None;
        slot.state = ProsodyState::Push;
        wake.notify_one();
        while slot.state == ProsodyState::Push {
            slot = wake.wait(slot).unwrap_or_else(|err| err.into_inner());
        }
        (slot.kept.take(), slot.turn)
    }
}

impl Drop for ProsodyEar {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slot;
        let mut slot = lock.lock().unwrap_or_else(|err| err.into_inner());
        slot.state = ProsodyState::Shutdown;
        wake.notify_one();
        drop(slot);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn prosody_loop(mut ear: horbm_ear::Ear, slot: Arc<(Mutex<ProsodySlot>, Condvar)>) {
    // One scorer for the life of the thread. 7881 has no ASR partial here.
    // That is prosody-only, not an empty transcript.
    let mut scorer = horbm_ear::Scorer::bundled();
    let (lock, wake) = &*slot;
    loop {
        let mut guard = lock.lock().unwrap_or_else(|err| err.into_inner());
        while guard.state == ProsodyState::Idle {
            guard = wake.wait(guard).unwrap_or_else(|err| err.into_inner());
        }
        match guard.state {
            ProsodyState::Shutdown => break,
            ProsodyState::Push => {
                let frame = guard.frame;
                let out = ear.push(&frame);
                guard.turn = scorer.push_prosody(out);
                guard.kept = heard_point(out);
                guard.state = ProsodyState::Idle;
                wake.notify_one();
            }
            ProsodyState::Idle => {}
        }
    }
}

fn heard_point(out: horbm_ear::FrameOut) -> Option<horbm::Heard> {
    let horbm_ear::FrameOut::Heard(heard) = out else {
        return None;
    };
    let (valence, arousal, strength) = heard.offer.heard_axes()?;
    horbm::Heard::from_axes(valence, arousal, strength)
}

/// `ear=on` or `ear=off`. Startup and the first `info()` call print this.
pub(crate) fn ear_state_line(sensing: bool) -> &'static str {
    if sensing {
        "ear=on"
    } else {
        "ear=off"
    }
}

/// Scripted recogniser and an explicit prosody ear. The process environment
/// is not read. The mind records each point the brain turn receives.
pub(crate) fn open_scripted_ear(
    transcript: &str,
    sense_on: bool,
    trace: Arc<crate::brain::EarTrace>,
) -> LivePipeline {
    let mind = HorbmMind::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge"))
        .with_trace(trace);
    let prosody = if sense_on {
        ProsodyBoot::ExplicitOn
    } else {
        ProsodyBoot::ExplicitOff
    };
    LivePipeline::assemble(
        Opened {
            vad: OpenVad {
                front: Front::Energy(EnergyGate::new(HANG_MS)),
                backend: "energy",
                ready: true,
            },
            asr: OpenAsr {
                ear: Ear::Script(transcript.to_string()),
                name: "script",
                worker: None,
            },
        },
        Arc::new(Mutex::new(mind)),
        None,
        Box::new(ScriptMouth),
        native::BOOT_MOUTH_DEVICE,
        prosody,
    )
}

struct ScriptMouth;

impl Mouth for ScriptMouth {
    fn speak(&mut self, _text: &str, _cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        Ok((vec![1000, -1000, 1000, -1000], crate::audio::SPEAKER_RATE))
    }
    fn mouth(&self) -> &'static str {
        "sample"
    }
    fn voice(&self) -> &'static str {
        "sample"
    }
    fn clone_status(&self) -> &'static str {
        "off"
    }
}

struct ProsodyBrain {
    inner: MindBrain,
    pending: Arc<Mutex<Option<horbm::Heard>>>,
}

impl TurnBrain for ProsodyBrain {
    fn turn(&mut self, heard: &str) -> SpokenLine {
        self.inner.turns.fetch_add(1, Ordering::Relaxed);
        let ear = self
            .pending
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        let started = Instant::now();
        let result = self
            .inner
            .mind
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .turn_heard(heard, ear);
        match result {
            Err(code) => {
                *self
                    .inner
                    .last
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) = Some(Stored::Err(code));
                quiet_line()
            }
            Ok(mut out) => {
                if out.took_us == 0 {
                    out.took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
                }
                out.mood = out.mood.finite_or_resting();
                let speak = may_speak_piper(&out.reply, out.violations);
                let line = line_from_turn(&out, speak);
                *self
                    .inner
                    .last
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) = Some(Stored::Out(out));
                line
            }
        }
    }
}

fn line_from_turn(out: &TurnOut, speak: bool) -> SpokenLine {
    SpokenLine {
        text: if speak {
            out.reply.clone()
        } else {
            String::new()
        },
        mood: horbm::Pad {
            arousal: out.mood.arousal,
            valence: out.mood.valence,
            dominance: out.mood.dominance,
        },
        rate: out.prosody_rate,
        energy: out.prosody_energy,
        pitch_st: out.prosody_pitch_st,
        pause_ms: out.prosody_pause_ms,
    }
}

fn quiet_line() -> SpokenLine {
    SpokenLine {
        text: String::new(),
        mood: horbm::Pad::RESTING,
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    }
}

/// First 80 ms from [`galion_sam_orch::FakeTts`], then the gate holds the rest.
struct GatedMouth {
    inner: galion_sam_orch::FakeTts,
}

impl Mouth for GatedMouth {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        let mut pcm = Vec::new();
        let cancel = AtomicBool::new(false);
        self.speak_stream(text, cue, &cancel, &mut |chunk| {
            pcm.extend_from_slice(chunk);
            true
        })?;
        Ok((pcm, SPEAKER_RATE))
    }

    fn speak_stream(
        &mut self,
        text: &str,
        cue: &MouthCue,
        cancel: &AtomicBool,
        on_pcm: &mut dyn FnMut(&[i16]) -> bool,
    ) -> Result<(), String> {
        let pad = galion_sam_orch::Pad {
            arousal: cue.mood.arousal,
            valence: cue.mood.valence,
            dominance: cue.mood.dominance,
        };
        OrchTts::speak(&mut self.inner, text, &pad, cancel, &mut |pcm| {
            let _ = on_pcm(pcm);
        });
        Ok(())
    }

    fn mouth(&self) -> &'static str {
        "fake"
    }
    fn voice(&self) -> &'static str {
        "fake"
    }
    fn clone_status(&self) -> &'static str {
        "off"
    }
}

struct GatedPipeline {
    mouth: SpeakWorker,
    reply: String,
    phase: &'static str,
}

impl GatedPipeline {
    fn new(gate: Arc<galion_sam_orch::voice_traits::WaitGate>) -> Self {
        let mouth = GatedMouth {
            inner: galion_sam_orch::FakeTts::gated(
                vec![2_000; crate::audio::SPEAKER_FRAME_SAMPLES],
                gate,
            ),
        };
        Self {
            mouth: SpeakWorker::spawn(Box::new(mouth)),
            reply: "The kettle is on. It can wait.".into(),
            phase: "idle",
        }
    }
}

impl Pipeline for GatedPipeline {
    fn push_mic(&mut self, _pcm: &[i16], _rate: u32, _t_ms: u64) -> TurnProduct {
        TurnProduct::events(vec![error_json("mic-unwired")])
    }

    fn push_text(&mut self, text: &str, _t_ms: u64) -> TurnProduct {
        if text.trim().is_empty() {
            self.phase = "idle";
            return TurnProduct::events(vec![error_json("empty-text"), dot_json("idle", 0.0)]);
        }
        let out = TurnOut {
            reply: self.reply.clone(),
            intent_json: "null".into(),
            arm_json: "null".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: vec!["fixture".into()],
            violations: 0,
            lookup_us: 1,
            took_us: 1,
            mood: Pad::RESTING,
            empathy_e: 0.0,
            prosody_rate: 1.0,
            prosody_energy: 1.0,
            prosody_pitch_st: 0.0,
            prosody_pause_ms: 0.0,
            first_clause: String::new(),
            rest: String::new(),
        };
        self.phase = "speaking";
        let feed = self.mouth.start(
            self.reply.clone(),
            MouthCue {
                mood: Pad::RESTING,
                rate: 1.0,
                energy: 1.0,
                pitch_st: 0.0,
                pause_ms: 0.0,
            },
            String::new(),
            String::new(),
        );
        TurnProduct {
            events: vec![turn_json(&out), dot_json("speaking", 0.0)],
            pcm: Vec::new(),
            rate: SPEAKER_RATE,
            reply: self.reply.clone(),
            feed: Some(feed),
        }
    }

    fn stop(&mut self) {
        self.mouth.cancel();
        self.phase = "idle";
    }

    fn utterance_done(&mut self) {
        if self.phase == "speaking" {
            self.phase = "idle";
        }
    }

    fn tick_mood(&mut self) -> (Pad, &'static str) {
        (Pad::RESTING, "resting")
    }

    fn phase(&self) -> &'static str {
        self.phase
    }

    fn level(&self) -> f32 {
        0.0
    }

    fn info(&self) -> PipeInfo {
        PipeInfo {
            ear: "unwired",
            mouth: self.mouth.mouth(),
            voice: self.mouth.voice(),
            clone_status: self.mouth.clone_status(),
            mouth_device: native::BOOT_MOUTH_DEVICE,
            mind: native::BOOT_BRAIN.into(),
            vad_backend: "energy",
            vad_ready: true,
            knowledge_cards: 0,
            knowledge_dir: String::new(),
        }
    }
}

/// The first slice is the fake mouth's opening PCM. The gate holds the rest.
#[doc(hidden)]
pub fn gated_mouth_factory(
    gate: Arc<galion_sam_orch::voice_traits::WaitGate>,
) -> Arc<dyn Fn() -> Box<dyn Pipeline> + Send + Sync> {
    Arc::new(move || Box::new(GatedPipeline::new(Arc::clone(&gate))))
}

fn open_vad() -> OpenVad {
    let require = std::env::var("GALION_VAD_REQUIRE").ok().as_deref() == Some("silero");
    if let Ok(bin) = std::env::var("GALION_VAD_BIN") {
        let path = PathBuf::from(bin.trim());
        if path.is_file() {
            if let Ok(child) = ChildVad::spawn(&path) {
                let backend = child.backend;
                let ready = !require || backend == "silero";
                return OpenVad {
                    front: Front::Child(child),
                    backend,
                    ready,
                };
            }
        }
    }
    let ready = !require;
    OpenVad {
        front: Front::Energy(EnergyGate::new(HANG_MS)),
        backend: "energy",
        ready,
    }
}

fn open_ear() -> OpenAsr {
    if galion_sam_asr::find_vosk_model().is_err() || galion_sam_asr::find_vosk_lib().is_none() {
        return OpenAsr {
            ear: Ear::Missing,
            name: "missing",
            worker: None,
        };
    }
    let (tx, rx) = mpsc::channel::<Job>();
    let worker = std::thread::spawn(move || {
        let opened = open_engine(&EngineOpts::backend("vosk")).ok();
        let mut engine = opened.map(|(engine, _, _)| engine);
        while let Ok(job) = rx.recv() {
            let result = match engine.as_mut() {
                None => Err("asr-unready".into()),
                Some(engine) => transcribe(engine.as_mut(), &job.pcm),
            };
            let _ = job.back.send(result);
        }
    });
    OpenAsr {
        ear: Ear::Vosk(tx),
        name: "vosk",
        worker: Some(worker),
    }
}

fn transcribe(engine: &mut dyn Asr, pcm: &[i16]) -> Result<String, String> {
    engine.reset().map_err(|_| "asr-unready".to_string())?;
    engine
        .push_pcm(pcm)
        .map_err(|_| "asr-unready".to_string())?;
    let events = engine.finish().map_err(|_| "asr-unready".to_string())?;
    let text = final_text(&events);
    let text = text.trim();
    if text.is_empty() {
        Err("asr-empty".into())
    } else {
        Ok(text.to_string())
    }
}

struct ChildVad {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    backend: &'static str,
}

impl ChildVad {
    fn spawn(bin: &Path) -> Result<Self, ()> {
        if bin.extension().and_then(|s| s.to_str()) == Some("py") {
            for cmd in py_invocations(bin) {
                if let Ok(child) = Self::from_cmd(cmd) {
                    return Ok(child);
                }
            }
            return Err(());
        }
        Self::from_cmd(Command::new(bin))
    }

    fn from_cmd(mut cmd: Command) -> Result<Self, ()> {
        let hang = std::env::var("GALION_VAD_HANG_MS").unwrap_or_else(|_| "640".into());
        let commit = std::env::var("GALION_VAD_COMMIT_MS").unwrap_or_else(|_| "640".into());
        cmd.env("GALION_VAD_HANG_MS", hang)
            .env("GALION_VAD_COMMIT_MS", commit)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|_| ())?;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => return reap(child),
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => return reap(child),
        };
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        if stdout.read_line(&mut line).is_err() {
            return reap(child);
        }
        let Some(backend) = parse_ready(&line) else {
            return reap(child);
        };
        Ok(Self {
            child,
            stdin,
            stdout,
            backend,
        })
    }

    fn push(&mut self, frame: &[i16]) -> Option<&'static str> {
        let hex = encode_frame(frame);
        if writeln!(self.stdin, "frame {hex}").is_err() || self.stdin.flush().is_err() {
            return None;
        }
        let mut line = String::new();
        if self.stdout.read_line(&mut line).is_err() {
            return None;
        }
        parse_score(&line).map(|score| score.event)
    }

    fn reset(&mut self) {
        let _ = writeln!(self.stdin, "reset");
        let _ = self.stdin.flush();
        let mut line = String::new();
        let _ = self.stdout.read_line(&mut line);
    }
}

/// `python3` on Linux, `python` or `py -3` on Windows. The first that prints
/// a ready line is the child. A missing name fails at `spawn` and the next
/// is tried.
fn py_invocations(script: &Path) -> Vec<Command> {
    let mut cmds = Vec::new();
    for name in ["python3", "python"] {
        let mut cmd = Command::new(name);
        cmd.arg(script);
        cmds.push(cmd);
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("py");
        cmd.arg("-3");
        cmd.arg(script);
        cmds.push(cmd);
    }
    cmds
}

fn reap<T>(mut child: Child) -> Result<T, ()> {
    let _ = child.kill();
    let _ = child.wait();
    Err(())
}

impl Drop for ChildVad {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
    }
}

fn encode_frame(frame: &[i16]) -> String {
    let mut out = String::with_capacity(frame.len() * 4);
    for sample in frame {
        for byte in sample.to_le_bytes() {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

fn parse_ready(line: &str) -> Option<&'static str> {
    let rest = line.trim().strip_prefix("ready backend=")?;
    match rest.split_whitespace().next()? {
        "silero" => Some("silero"),
        "energy" => Some("energy"),
        _ => None,
    }
}

struct Score {
    event: &'static str,
}

fn parse_score(line: &str) -> Option<Score> {
    let line = line.trim();
    if !line.starts_with("score ") {
        return None;
    }
    let mut event = None;
    for part in line.split_whitespace().skip(1) {
        if let Some(value) = part.strip_prefix("event=") {
            event = Some(match value {
                "start" => "start",
                "end" => "end",
                _ => "none",
            });
        }
    }
    Some(Score { event: event? })
}

fn frame_16k(frame: &[i16], rate: u32) -> Vec<i16> {
    if rate == 24_000 {
        downsample_24_to_16(frame)
    } else {
        frame.to_vec()
    }
}

fn downsample_24_to_16(pcm: &[i16]) -> Vec<i16> {
    if pcm.is_empty() {
        return Vec::new();
    }
    let out_len = pcm.len() * 2 / 3;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * 1.5;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(pcm.len() - 1);
        let frac = pos - i0 as f64;
        let s = f64::from(pcm[i0]) * (1.0 - frac) + f64::from(pcm[i1]) * frac;
        out.push(s.round().clamp(-32768.0, 32767.0) as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::FeedMsg;
    use std::io::Write;

    #[test]
    fn ready_and_score_lines() {
        assert_eq!(parse_ready("ready backend=silero\n"), Some("silero"));
        assert_eq!(parse_ready("ready backend=energy\n"), Some("energy"));
        assert!(parse_ready("ready backend=other").is_none());
        assert_eq!(
            parse_score("score prob=0.800 speaking=1 event=start")
                .unwrap()
                .event,
            "start"
        );
        assert_eq!(
            parse_score("score prob=0.100 speaking=0 event=end")
                .unwrap()
                .event,
            "end"
        );
    }

    #[test]
    fn text_miss_is_i_dont_know_and_the_mouth_is_called() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(heard.clone(), Ear::Script("unused".into()), "script");
        let mut out = pipe.push_text("who won the match", 0);
        out.drain_audio();
        assert!(out.events.iter().all(|e| !e.contains("brain-unlinked")));
        assert_eq!(out.reply, "I don't know.");
        assert!(!out.pcm.is_empty());
        assert_eq!(heard.lock().unwrap().as_slice(), ["I don't know."]);
        assert_eq!(pipe.voice_turns(), 1);
        let mut kettle = pipe.push_text("where is the kettle", 1);
        kettle.drain_audio();
        assert_eq!(kettle.reply, "The kettle is on the stove.");
        assert!(kettle.sources_ok());
        assert_eq!(pipe.voice_turns(), 2);
    }

    #[test]
    fn mic_commit_runs_vad_then_the_card() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(
            heard.clone(),
            Ear::Script("where is the kettle".into()),
            "script",
        );
        assert!(pipe.vad_ready);
        assert_eq!(pipe.vad_backend, "energy");
        let pcm = vec![8_000i16; FRAME * 8];
        let mut out = pipe.commit_pcm(&pcm, 16_000, 0);
        out.drain_audio();
        assert!(out.events.iter().any(|e| e.contains("\"type\":\"vad\"")));
        assert_eq!(out.reply, "The kettle is on the stove.");
        assert!(!out.pcm.is_empty());
        assert!(out.events.iter().all(|e| !e.contains("brain-unlinked")));
    }

    #[test]
    fn streaming_mic_waits_640_ms_then_speaks() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(
            heard.clone(),
            Ear::Script("where is the kettle".into()),
            "script",
        );
        let loud = vec![8_000i16; FRAME];
        let quiet = vec![0i16; FRAME];
        for _ in 0..4 {
            let out = pipe.push_mic(&loud, 16_000, 0);
            assert!(out.pcm.is_empty());
        }
        for _ in 0..31 {
            let mid = pipe.push_mic(&quiet, 16_000, 0);
            assert!(mid.pcm.is_empty(), "{}", mid.events.join("\n"));
            assert!(mid.events.iter().all(|e| !e.contains("\"event\":\"end\"")));
        }
        let mut end = pipe.push_mic(&quiet, 16_000, 0);
        end.drain_audio();
        assert!(end.events.iter().any(|e| e.contains("\"event\":\"end\"")));
        assert_eq!(end.reply, "The kettle is on the stove.");
        assert!(!end.pcm.is_empty());
        assert!(end.events.iter().all(|e| !e.contains("brain-unlinked")));
        assert_eq!(
            heard.lock().unwrap().as_slice(),
            ["The kettle is on the stove."]
        );
        assert_eq!(pipe.voice_turns(), 1);
    }

    #[test]
    fn first_sentence_is_emitted_before_the_next_one_starts() {
        let cancel = AtomicBool::new(false);
        let second = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&second);
        let mut n = 0u32;
        let mut first_before_second = false;
        crate::mouth::speak_pieces(
            "The kettle is on. It can wait.",
            &cancel,
            &mut |piece, emit| {
                if piece.contains("wait") {
                    flag.store(true, Ordering::SeqCst);
                }
                assert!(emit(&vec![2_000; crate::audio::SPEAKER_FRAME_SAMPLES]));
                Ok(())
            },
            &mut |_pcm| {
                n += 1;
                if n == 1 {
                    first_before_second = !flag.load(Ordering::SeqCst);
                }
                true
            },
        )
        .unwrap();
        assert!(first_before_second);
        assert_eq!(n, 2);
        assert!(second.load(Ordering::SeqCst));
    }

    #[test]
    fn first_clause_is_handed_over_before_the_rest_of_the_sentence() {
        let cancel = AtomicBool::new(false);
        let mut heard = Vec::new();
        crate::mouth::speak_pieces(
            "AlphaFold predicts protein structure, and the twenty twenty one paper showed it at scale.",
            &cancel,
            &mut |piece, emit| {
                heard.push(piece.to_string());
                assert!(emit(&[1, 2, 3, 4]));
                Ok(())
            },
            &mut |_pcm| true,
        )
        .unwrap();
        assert!(heard.len() >= 2, "{heard:?}");
        assert_eq!(heard[0], "AlphaFold predicts protein structure,");
        assert!(heard[1].starts_with("and "));
        let joined = heard.join(" ");
        assert_eq!(
            joined,
            "AlphaFold predicts protein structure, and the twenty twenty one paper showed it at scale."
        );
    }

    #[test]
    fn cancel_after_the_first_clause_does_not_synthesize_the_rest() {
        let cancel = AtomicBool::new(false);
        let mut heard = Vec::new();
        crate::mouth::speak_pieces(
            "AlphaFold predicts protein structure, and the twenty twenty one paper showed it at scale.",
            &cancel,
            &mut |piece, emit| {
                heard.push(piece.to_string());
                cancel.store(true, Ordering::SeqCst);
                let _ = emit(&[1]);
                Ok(())
            },
            &mut |_pcm| true,
        )
        .unwrap();
        assert_eq!(
            heard,
            vec!["AlphaFold predicts protein structure,".to_string()]
        );
    }

    #[test]
    fn first_slice_is_queued_before_the_fake_mouth_finishes() {
        let gate = galion_sam_orch::voice_traits::WaitGate::new();
        let mut pipe = GatedPipeline::new(Arc::clone(&gate));
        let started = std::time::Instant::now();
        let mut out = pipe.push_text("hello", 0);
        let ready_us = started.elapsed().as_micros();
        let feed = out.feed.as_mut().expect("stream");
        let first = recv_feed(&mut feed.rx, Duration::from_secs(2)).expect("first slice");
        let slice_us = started.elapsed().as_micros();
        let FeedMsg::Slice(pcm) = first else {
            panic!("expected a slice");
        };
        assert_eq!(pcm.len(), crate::audio::SPEAKER_FRAME_SAMPLES);
        assert!(!gate.release.load(Ordering::SeqCst));
        let quiet = std::time::Instant::now() + Duration::from_millis(40);
        while std::time::Instant::now() < quiet {
            assert!(
                matches!(
                    feed.rx.try_recv(),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty)
                ),
                "the rest of the reply was synthesised before the gate opened"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        eprintln!(
            "MEASURE vm fake-mouth reply_ready_us={ready_us} reply_ready_to_first_slice_us={}",
            slice_us.saturating_sub(ready_us)
        );
        gate.release.store(true, Ordering::SeqCst);
        let rest = recv_feed(&mut feed.rx, Duration::from_secs(2)).expect("rest");
        let FeedMsg::Slice(tail) = rest else {
            panic!("expected the held tail");
        };
        assert_eq!(tail, vec![9, 9, 9, 9]);
        drop(out);
        let joined = std::time::Instant::now();
        drop(pipe);
        assert!(joined.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn stop_unblocks_the_gated_fake_mouth() {
        let gate = galion_sam_orch::voice_traits::WaitGate::new();
        let mut pipe = GatedPipeline::new(gate);
        let mut out = pipe.push_text("hello", 0);
        let feed = out.feed.as_mut().expect("stream");
        let _ = recv_feed(&mut feed.rx, Duration::from_secs(2)).expect("first slice");
        let started = std::time::Instant::now();
        pipe.stop();
        drop(out);
        drop(pipe);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the mouth thread stayed parked after cancel"
        );
    }

    fn recv_feed(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<FeedMsg>,
        timeout: Duration,
    ) -> Option<FeedMsg> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match rx.try_recv() {
                Ok(msg) => return Some(msg),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => return None,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    if std::time::Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    #[test]
    fn missing_vosk_does_not_invent_a_line() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(heard.clone(), Ear::Missing, "missing");
        let pcm = vec![8_000i16; FRAME * 8];
        let out = pipe.commit_pcm(&pcm, 16_000, 0);
        assert!(out.pcm.is_empty());
        assert!(out.events.iter().any(|e| e.contains("asr-unready")));
        assert!(heard.lock().unwrap().is_empty());
    }

    #[test]
    fn vad_child_reports_silero_ready() {
        let path = write_ear_script();
        match ChildVad::spawn(&path) {
            Ok(mut child) => {
                assert_eq!(child.backend, "silero");
                let event = child.push(&[1_000i16; FRAME]);
                assert_eq!(event, Some("start"));
                drop(child);
            }
            Err(()) => {
                eprintln!(
                    "vad child skipped: no python interpreter; the energy ear is the default"
                );
                assert_energy_is_the_default_ear();
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_interpreter_keeps_the_energy_ear() {
        let missing = ChildVad::from_cmd(Command::new("galion-no-such-python"));
        assert!(missing.is_err());
        assert_energy_is_the_default_ear();
    }

    fn assert_energy_is_the_default_ear() {
        if std::env::var_os("GALION_VAD_BIN").is_some() {
            eprintln!("energy default not asserted: GALION_VAD_BIN is already set");
            return;
        }
        let opened = open_vad();
        assert!(
            matches!(opened.front, Front::Energy(_)),
            "a missing child must use the energy ear"
        );
        assert_eq!(opened.backend, "energy");
        let require = std::env::var("GALION_VAD_REQUIRE").ok().as_deref() == Some("silero");
        assert_eq!(opened.ready, !require);
    }

    #[test]
    fn windows_script_stays_on_the_g_drive() {
        let text = include_str!("../../../scripts/windows/Start-GalionOne.ps1");
        for needle in [
            r"G:\galion\work\galion-one-voice",
            r"G:\galion\ci\toolchains\cargo",
            r"G:\galion\ci\toolchains\rustup",
            r"G:\galion\work\target-voice",
            r"G:\galion\tmp\win",
            r"G:\galion\work\logs",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "CARGO_TARGET_DIR",
            "HF_HOME",
            "PIP_CACHE_DIR",
            "NPM_CONFIG_CACHE",
            "127.0.0.1:7881",
            "silero_vad.onnx",
            "vosk-model-small-en-us-0.15",
            "en_US-ljspeech-medium",
            r"G:\galion\models\asr\vosk-model-small-en-us-0.15",
            r"G:\galion\models\asr\vosk-win64-0.3.45",
            r"voice-stack\vad\models\silero_vad.onnx",
            r"voice-stack\tts-piper-stream\var",
            r"G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-cpu-1.31.1",
            r"G:\galion\tmp\cache\ort.pyke.io",
            "ORT_DYLIB_PATH",
            "ORT_CACHE_DIR",
            "GALION_VAD_HANG_MS",
            "640",
        ] {
            assert!(text.contains(needle), "missing {needle}");
        }
        assert!(!text.contains("%USERPROFILE%"));
        assert!(!text.contains("%APPDATA%"));
        assert!(!text.contains("%LOCALAPPDATA%"));
        assert!(!text.contains("%TEMP%"));
        assert!(!text.contains("C:\\Users"));
        assert!(!text.to_ascii_lowercase().contains("ollama"));
        assert!(!text.to_ascii_lowercase().contains("openai"));
        let cmd = include_str!("../../../scripts/windows/START-SAMANTHA-7881.cmd");
        for needle in [
            r"G:\galion\models\asr\vosk-model-small-en-us-0.15",
            r"G:\galion\models\asr\vosk-win64-0.3.45",
            r"voice-stack\vad\models\silero_vad.onnx",
            r"voice-stack\tts-piper-stream\var",
            r"onnxruntime-win-x64-cpu-1.31.1",
            r"G:\galion\tmp\cache\ort.pyke.io",
            "ORT_DYLIB_PATH",
            "GALION_VAD_HANG_MS=640",
            "127.0.0.1:7881",
            "I don't know.",
        ] {
            assert!(cmd.contains(needle), "cmd missing {needle}");
        }
        assert!(!cmd.contains("%USERPROFILE%"));
        assert!(!cmd.contains("%APPDATA%"));
        assert!(!cmd.contains("%LOCALAPPDATA%"));
        assert!(!cmd.contains("%TEMP%"));
        assert!(!cmd.contains("C:\\Users"));
        assert!(!cmd.to_ascii_lowercase().contains("ollama"));
        assert!(!cmd.to_ascii_lowercase().contains("openai"));
    }

    fn test_pipe(heard: Arc<Mutex<Vec<String>>>, ear: Ear, ear_name: &'static str) -> LivePipeline {
        LivePipeline::for_test(
            Front::Energy(EnergyGate::new(HANG_MS)),
            "energy",
            true,
            ear,
            ear_name,
            HorbmMind::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")),
            Box::new(RecMouth {
                heard,
                cues: Arc::new(Mutex::new(Vec::new())),
            }),
        )
    }

    #[test]
    fn empathy_prosody_reaches_the_mouth() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let cues = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = LivePipeline::for_test(
            Front::Energy(EnergyGate::new(HANG_MS)),
            "energy",
            true,
            Ear::Script("unused".into()),
            "script",
            HorbmMind::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")),
            Box::new(RecMouth {
                heard,
                cues: cues.clone(),
            }),
        );
        let mut out = pipe.push_text("I am so tired", 0);
        out.drain_audio();
        let turn = out
            .events
            .iter()
            .find(|event| event.contains("\"type\":\"turn\""))
            .expect("turn");
        let parsed: serde_json::Value = serde_json::from_str(turn).unwrap();
        let cue = cues.lock().unwrap()[0];
        let prosody = &parsed["prosody"];
        assert!((f64::from(cue.rate) - prosody["rate"].as_f64().unwrap()).abs() < 1e-4);
        assert!((f64::from(cue.energy) - prosody["energy"].as_f64().unwrap()).abs() < 1e-4);
        assert!((f64::from(cue.pitch_st) - prosody["pitch_st"].as_f64().unwrap()).abs() < 1e-4);
        assert!((f64::from(cue.pause_ms) - prosody["pause_ms"].as_f64().unwrap()).abs() < 1e-4);
        let mood = &parsed["mood"];
        assert!((f64::from(cue.mood.arousal) - mood["arousal"].as_f64().unwrap()).abs() < 1e-4);
        assert!((f64::from(cue.mood.valence) - mood["valence"].as_f64().unwrap()).abs() < 1e-4);
        assert!((f64::from(cue.mood.dominance) - mood["dominance"].as_f64().unwrap()).abs() < 1e-4);
        assert!(parsed["empathy_e"].as_f64().unwrap() > 0.0);
    }

    fn write_ear_script() -> PathBuf {
        let path = std::env::temp_dir().join(format!("galion-vad-ear-{}.py", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "#!/usr/bin/env python3\nimport sys\nprint('ready backend=silero', flush=True)\n\
for line in sys.stdin:\n    line=line.strip()\n    \
if line.startswith('frame'):\n        print('score prob=0.800 speaking=1 event=start', flush=True)\n    \
elif line=='quit':\n        break\n"
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata().unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    impl TurnProduct {
        fn sources_ok(&self) -> bool {
            self.events.iter().any(|e| e.contains("card:kettle"))
        }
    }

    fn native_pipe() -> LivePipeline {
        LivePipeline::for_test(
            Front::Energy(EnergyGate::new(HANG_MS)),
            "energy",
            true,
            Ear::Script("unused".into()),
            "script",
            HorbmMind::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")),
            Box::new(crate::mouth::NativeMouth::new()),
        )
    }

    fn turn_replies(events: &[String]) -> Vec<String> {
        events
            .iter()
            .filter_map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).ok()?;
                if value.get("type").and_then(|kind| kind.as_str()) == Some("turn") {
                    value
                        .get("reply")
                        .and_then(|reply| reply.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn native_mouth_puts_the_clauses_on_the_feed_and_returns_no_pcm() {
        let mut pipe = native_pipe();
        assert_eq!(pipe.info().mouth, "native");
        assert_eq!(pipe.info().voice, "text");
        let mut hit = pipe.push_text("where is the kettle", 0);
        let reply = hit.reply.clone();
        hit.drain_audio();
        assert!(hit.pcm.is_empty());
        let spoken = turn_replies(&hit.events);
        let (first, rest) = crate::mouth::clause_pair(&reply);
        assert_eq!(spoken, vec![first, rest]);
        assert!(hit.events.iter().all(|event| !event.contains("piper")));
        let mut miss = pipe.push_text("who won the match", 1);
        miss.drain_audio();
        assert!(miss.pcm.is_empty());
        assert_eq!(
            turn_replies(&miss.events),
            vec!["I don't know.".to_string()]
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_mouth_text_is_on_the_socket_and_no_pcm_follows() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::Message;

        let factory: crate::server::PipelineFactory = Arc::new(|| Box::new(native_pipe()));
        let running = crate::server::start(
            "127.0.0.1:0".parse().unwrap(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"),
            factory,
            crate::RefState::Unset,
            false,
            Duration::from_secs(30),
        )
        .await
        .unwrap();
        let health = tokio::net::TcpStream::connect(running.addr).await.unwrap();
        let (mut health_read, mut health_write) = health.into_split();
        tokio::io::AsyncWriteExt::write_all(
            &mut health_write,
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
        let mut body = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut health_read, &mut body)
            .await
            .unwrap();
        let health_text = String::from_utf8_lossy(&body);
        assert!(health_text.contains("\"mouth\":\"native\""));
        assert!(health_text.contains("\"voice\":\"text\""));

        let mut request = format!("ws://localhost:{}/v1/samantha", running.addr.port())
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("Origin", "http://127.0.0.1:7878".parse().unwrap());
        let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        ws.send(Message::Text(
            "{\"schema\":\"galion-samantha-ws/1\",\"type\":\"hello\",\"session\":null}".into(),
        ))
        .await
        .unwrap();
        let welcome = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(welcome) = welcome else {
            panic!("welcome was not text");
        };
        assert!(welcome.contains("\"type\":\"welcome\""));
        ws.send(Message::Text(
            "{\"schema\":\"galion-samantha-ws/1\",\"type\":\"text\",\"text\":\"where is the kettle\"}"
                .into(),
        ))
        .await
        .unwrap();

        let mut replies = Vec::new();
        let mut pcm = 0usize;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && replies.len() < 2 {
            let Some(Ok(msg)) = tokio::time::timeout(Duration::from_millis(400), ws.next())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            match msg {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if value.get("type").and_then(|kind| kind.as_str()) == Some("turn") {
                            if let Some(reply) = value.get("reply").and_then(|r| r.as_str()) {
                                replies.push(reply.to_string());
                            }
                        }
                        assert_ne!(
                            value.get("error").and_then(|e| e.as_str()),
                            Some("piper-missing")
                        );
                    }
                }
                Message::Binary(_) => pcm += 1,
                _ => {}
            }
        }
        assert_eq!(pcm, 0, "native mouth sends no PCM");
        assert_eq!(
            replies,
            vec!["The kettle".to_string(), "is on the stove.".to_string()]
        );
    }
}
