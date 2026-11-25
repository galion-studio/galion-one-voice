// Galion Voice — Perpetual Open MIT License
//! S7 · phone turn worker (feature `phone-turn`): one long-lived thread off the 80 ms
//! heartbeat (TURN-WORKER.md). The heartbeat `try_submit`s a finished utterance and
//! `try_recv`s the reply each tick; it never blocks on model work.
//!
//! Pipeline: [`galion_cascade::PhoneLane`] (S4 ASR → S5 LLM → S6 TTS), built once from env
//! inside the worker thread (`GALION_ASR`, `GALION_LLM`, `GALION_SPEECH`). S4 is Vosk when
//! `GALION_ASR=vosk` and the small-en model is on disk (`asr=vosk`). The length-bucket
//! placeholder is not selected by the environment. Errors (`AsrUnratified`, `AsrUnavailable`,
//! `NoSpeech`) are reported in [`TurnReply::error`] and a clear two-tone WAV is published
//! instead, so the phone still hears that its turn arrived. The tone is not a transcript.
//! `GALION_SPEECH=off` is that path: two-tone WAV, `backend=tone`, `error` set.
//!
//! The job slot is the V2-S1 bus lane ([`crate::bus::JOB_DEPTH`] = 1, `try_send` only).
//! Cancel is [`crate::bus::ProcessBus::supersede`]. A stale `turn_id` is dropped before
//! ASR and again before the reply is handed back. The same token drops Piper clauses
//! that have not started. The thread and the Piper process are not killed.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::time::Instant;

use std::sync::{Arc, Mutex};

use galion_cascade::en_lane::{wav_bytes, LANE_RATE_HZ};
use galion_cascade::phone::FrameProsody;
use galion_cascade::{LlmMode, PhoneConfig, PhoneError, PhoneLane};
use galion_openmind::{seam_enabled_from_env, ReplyFrom, TurnSeam};

use crate::bus::{
    bytes_to_pcm16, pcm16_to_bytes, BusBody, BusGate, BusMsg, ProcessBus, WORKER_REPLY_DEPTH,
};

/// Latest 80 ms prosody, written by the screen loop and read when a turn speaks.
/// The heartbeat does not lock this. Identity until the host stores a frame.
#[derive(Clone)]
pub struct ProsodySlot {
    inner: Arc<Mutex<FrameProsody>>,
}

impl ProsodySlot {
    pub fn identity() -> Self {
        Self { inner: Arc::new(Mutex::new(FrameProsody::identity())) }
    }

    pub fn store(&self, prosody: FrameProsody) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = prosody;
        }
    }

    pub fn load(&self) -> FrameProsody {
        self.inner.lock().map(|slot| *slot).unwrap_or_else(|_| FrameProsody::identity())
    }
}

/// One utterance handed to the worker: mono PCM16 at `rate` (the lane resamples to 16 kHz).
pub struct TurnJob {
    pub id: u64,
    pub rate: u32,
    pub pcm: Vec<i16>,
    /// Set only for a turn whose HTTP host was `voice.galion.app`.
    /// `galion.app` (the One page) stays false, so a research cue is not spoken from there.
    pub voice_stack: bool,
}

/// Spoken research reply. The closure is the desk behind `POST /research/ask`.
pub type ResearchAsk = std::sync::Arc<dyn Fn(&str) -> String + Send + Sync>;

impl TurnJob {
    /// Overwrite the utterance. Also run from [`Drop`].
    pub fn wipe(&mut self) {
        for s in &mut self.pcm {
            *s = 0;
        }
        std::hint::black_box(self.pcm.as_slice());
    }
}

impl Drop for TurnJob {
    fn drop(&mut self) {
        self.wipe();
    }
}

/// Finished reply: mono PCM16 at [`LANE_RATE_HZ`] (24 kHz) + its WAV bytes.
pub struct TurnReply {
    pub id: u64,
    /// S4 transcript (placeholder when `asr == "interim-placeholder"`).
    pub transcript: String,
    pub text: String,
    /// 24 kHz mono PCM16 of `wav` (drives the me band).
    pub pcm: Vec<i16>,
    pub wav: Vec<u8>,
    pub ms: u32,
    /// `a21-weights` | `system-flite` | `tone`.
    pub backend: &'static str,
    /// `stub-fixture` | `interim-placeholder` | `none` (error).
    pub asr: &'static str,
    pub llm: &'static str,
    /// `"asr=.. llm=.. tts=.."` stage timings.
    pub stages: String,
    /// Register in force when the turn was admitted. Not drawn, not a film voice.
    pub persona: &'static str,
    /// Set when the lane failed and the tone was published instead.
    pub error: Option<String>,
    pub in_ms: u32,
    pub took_ms: u32,
}

impl TurnReply {
    /// Overwrite PCM, the WAV bytes, and the transcript. Also run from [`Drop`].
    pub fn wipe(&mut self) {
        for s in &mut self.pcm {
            *s = 0;
        }
        for b in &mut self.wav {
            *b = 0;
        }
        std::hint::black_box(self.pcm.as_slice());
        std::hint::black_box(self.wav.as_slice());
        wipe_string(&mut self.transcript);
        wipe_string(&mut self.text);
    }
}

impl Drop for TurnReply {
    fn drop(&mut self) {
        self.wipe();
    }
}

fn wipe_string(s: &mut String) {
    let mut v = std::mem::take(s).into_bytes();
    for b in &mut v {
        *b = 0;
    }
    std::hint::black_box(v.as_slice());
}

/// Persona reply text to speak as-is (no ASR, no canned table).
pub struct SpeakJob {
    pub id: u64,
    pub text: String,
    /// Sensed `/v2/say` column. `None` keeps the latest 80 ms slot.
    pub prosody: Option<FrameProsody>,
}

impl Drop for SpeakJob {
    fn drop(&mut self) {
        wipe_string(&mut self.text);
    }
}

/// Handle owned by the heartbeat loop. The loop only calls [`Self::try_submit`],
/// [`Self::try_speak`], and [`Self::try_recv`]; all three return immediately.
pub struct TurnWorker {
    bus: ProcessBus,
    reply_rx: Receiver<TurnReply>,
    speak_tx: SyncSender<SpeakJob>,
    /// Turn id whose job is a voice-stack handoff. 0 when the lane holds none.
    voice_turn: Arc<AtomicU64>,
    ask: Arc<Mutex<Option<ResearchAsk>>>,
}

impl TurnWorker {
    /// Env-configured lane (`GALION_ASR`, `GALION_LLM`).
    pub fn spawn() -> Self {
        Self::spawn_with(phone_config_from_env())
    }

    /// Same worker with an explicit lane config.
    /// The presence-eval harness uses this so a barge-in probe does not mutate `GALION_ASR`.
    ///
    /// Returns only after the lane exists. Piper pre-warm runs there, so a turn
    /// submitted as soon as `spawn` returns is not stuck behind model load in
    /// the one-slot queue. The 80 ms loop never calls this.
    pub fn spawn_with(cfg: PhoneConfig) -> Self {
        Self::spawn_on_with(ProcessBus::new(), cfg, ProsodySlot::identity())
    }

    /// `bus` is shared with the host. The lane is read from the environment.
    /// The worker thread holds only the cancel gate, so dropping this handle
    /// disconnects the job lane and the thread exits.
    pub fn spawn_on(bus: ProcessBus) -> Self {
        Self::spawn_linked(bus, ProsodySlot::identity())
    }

    /// Same as [`spawn_on`] with the host's prosody slot. The 80 ms loop writes it.
    pub fn spawn_linked(bus: ProcessBus, prosody: ProsodySlot) -> Self {
        Self::spawn_on_with(bus, phone_config_from_env(), prosody)
    }

    fn spawn_on_with(bus: ProcessBus, cfg: PhoneConfig, prosody: ProsodySlot) -> Self {
        let job_rx = bus.take_job_rx().expect("job receiver taken once");
        let gate = bus.gate();
        let (rtx, reply_rx) = sync_channel::<TurnReply>(WORKER_REPLY_DEPTH);
        let (speak_tx, speak_rx) = sync_channel::<SpeakJob>(1);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        let voice_turn = Arc::new(AtomicU64::new(0));
        let voice_for_thread = Arc::clone(&voice_turn);
        let ask = Arc::new(Mutex::new(None));
        let ask_for_thread = Arc::clone(&ask);
        std::thread::Builder::new()
            .name("galion-turn-worker".into())
            .spawn(move || {
                let mut lane = PhoneLane::new(cfg);
                let _ = ready_tx.send(());
                loop {
                    match speak_rx.try_recv() {
                        Ok(job) => {
                            let turn_id = job.id;
                            if !gate.admit_asr(turn_id) {
                                continue;
                            }
                            let Some(reply) = respond_speak(&mut lane, job, &gate, &prosody) else {
                                let _ = gate.admit_publish(turn_id);
                                continue;
                            };
                            if !gate.admit_publish(reply.id) {
                                continue;
                            }
                            if rtx.try_send(reply).is_err() {
                                gate.note_worker_reply_dropped();
                            }
                            continue;
                        }
                        Err(TryRecvError::Disconnected) => break,
                        Err(TryRecvError::Empty) => {}
                    }
                    match job_rx.recv_timeout(std::time::Duration::from_millis(25)) {
                        Ok(msg) => {
                            let turn_id = msg.turn_id;
                            if !gate.admit_asr(turn_id) {
                                continue; // superseded (barge-in) before ASR
                            }
                            let mut job = turn_job_from(msg);
                            if voice_for_thread.load(Ordering::SeqCst) == job.id {
                                let _ = voice_for_thread.compare_exchange(job.id, 0, Ordering::SeqCst, Ordering::SeqCst);
                                job.voice_stack = true;
                            }
                            let Some(reply) = respond(&mut lane, job, &gate, &prosody, &ask_for_thread) else {
                                let _ = gate.admit_publish(turn_id);
                                continue; // superseded while synthesizing, before publish
                            };
                            if !gate.admit_publish(reply.id) {
                                continue;
                            }
                            if rtx.try_send(reply).is_err() {
                                // depth 4 and full, or the host is gone: drop, never block
                                gate.note_worker_reply_dropped();
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("spawn turn worker");
        ready_rx.recv().expect("turn worker stopped during lane start");
        Self { bus, reply_rx, speak_tx, voice_turn, ask }
    }

    /// Desk behind `POST /research/ask`. The voice stack calls it for a research cue.
    /// A missing asker speaks the local line. This does not run on the 80 ms tick.
    pub fn set_research_ask(&self, ask: ResearchAsk) {
        if let Ok(mut slot) = self.ask.lock() {
            *slot = Some(ask);
        }
    }

    pub fn bus(&self) -> &ProcessBus {
        &self.bus
    }

    /// Mark `id` as the newest turn whose reply is still wanted.
    /// Older in-flight work is dropped. The thread keeps running.
    pub fn supersede(&self, id: u64) {
        self.bus.supersede(id);
    }

    /// Speak `text` as the persona already wrote it. `Err(text)` when the slot is full.
    /// An older id does not rewind the cancel token.
    pub fn try_speak(&self, id: u64, text: String, prosody: Option<FrameProsody>) -> Result<(), String> {
        self.supersede(id);
        match self.speak_tx.try_send(SpeakJob { id, text, prosody }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(mut job) | TrySendError::Disconnected(mut job)) => {
                Err(std::mem::take(&mut job.text))
            }
        }
    }

    /// Non-blocking submit. `Err(job)` when the depth-1 job lane is full.
    /// An older id does not rewind the cancel token.
    pub fn try_submit(&self, job: TurnJob) -> Result<(), TurnJob> {
        self.supersede(job.id);
        let voice_stack = job.voice_stack;
        let id = job.id;
        if voice_stack {
            self.voice_turn.store(id, Ordering::SeqCst);
        } else if self.voice_turn.load(Ordering::SeqCst) == id {
            self.voice_turn.store(0, Ordering::SeqCst);
        }
        let msg = BusMsg::utterance(
            self.bus.alloc_id(),
            job.id,
            0,
            self.bus.cancel(),
            job.rate,
            pcm16_to_bytes(&job.pcm),
        );
        match self.bus.try_send_job(msg) {
            Ok(()) => Ok(()),
            Err(msg) => {
                if voice_stack && self.voice_turn.load(Ordering::SeqCst) == id {
                    self.voice_turn.store(0, Ordering::SeqCst);
                }
                let mut back = turn_job_from(msg);
                back.voice_stack = voice_stack;
                Err(back)
            }
        }
    }

    /// Non-blocking poll. A reply that went stale after it was queued is dropped.
    pub fn try_recv(&self) -> Option<TurnReply> {
        loop {
            match self.reply_rx.try_recv() {
                Ok(r) if self.bus.admit_publish(r.id) => return Some(r),
                Ok(_) => continue,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

fn phone_config_from_env() -> PhoneConfig {
    PhoneConfig::from_env().unwrap_or_else(|e| panic!("phone lane: {e}"))
}

fn turn_job_from(msg: BusMsg) -> TurnJob {
    let (rate, pcm) = match msg.body {
        BusBody::Utterance { rate, pcm } => (rate, bytes_to_pcm16(&pcm)),
        _ => (16_000, Vec::new()),
    };
    TurnJob {
        id: msg.turn_id,
        rate,
        pcm,
        voice_stack: false,
    }
}

fn stage_log(body: &str, stamps: &galion_cascade::Domain04Stamps) -> String {
    let line = stamps.log_line();
    debug_assert!(galion_cascade::stamp_log_clean(&line), "{line}");
    if body.is_empty() { line } else { format!("{body} {line}") }
}

fn respond_speak(lane: &mut PhoneLane, mut job: SpeakJob, gate: &BusGate, prosody: &ProsodySlot) -> Option<TurnReply> {
    lane.set_stamp_turn(job.id);
    let persona = gate.persona_id().as_str();
    let id = job.id;
    let delivery = job.prosody.unwrap_or_else(|| prosody.load());
    lane.set_delivery(persona, delivery);
    lane.arm_cancel(gate.cancel_token(), id);
    let t0 = Instant::now();
    let mut text = std::mem::take(&mut job.text);
    let result = lane.speak_fixed(&text);
    wipe_string(&mut text);
    drop(job);
    match result {
        Ok(t) => {
            let pcm = reply_pcm(&t.wav_bytes);
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id,
                transcript: String::new(),
                text: t.reply_text,
                pcm,
                wav: t.wav_bytes,
                ms,
                backend: t.speech_backend,
                asr: t.asr.id(),
                llm: "persona-floor",
                stages: stage_log(
                    &format!("tts={} tts_first={} first_audio={}", t.stage_ms.tts, t.stage_ms.tts_first_pcm, t.stage_ms.first_audio_ms),
                    &t.stamps,
                ),
                error: None,
                in_ms: 0,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
        Err(PhoneError::Superseded) => None,
        Err(PhoneError::Empty) => None,
        Err(e) => {
            let pcm = tone();
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id,
                transcript: String::new(),
                text: String::new(),
                wav: wav_bytes(&pcm, LANE_RATE_HZ),
                pcm,
                ms,
                backend: "tone",
                asr: "text-inject",
                llm: "persona-floor",
                stages: String::new(),
                error: Some(format!("NoSpeech: {e}")),
                in_ms: 0,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
    }
}

fn ask_research(slot: &Mutex<Option<ResearchAsk>>, question: &str) -> String {
    let f = slot.lock().ok().and_then(|g| g.clone());
    let spoken = match f {
        Some(f) => f(question),
        None => galion_cascade::research_handoff::RESEARCH_LOCAL.to_string(),
    };
    if spoken.trim().is_empty() {
        galion_cascade::research_handoff::RESEARCH_LOCAL.to_string()
    } else {
        spoken
    }
}

/// Voice-stack turn. A research cue is handed to `/research/ask` and spoken
/// back as [`galion_cascade::research_handoff::RESEARCH_VOICE`]. The mouth is
/// the lane already running: no new voice model. Any other transcript stays
/// on the ordinary reply, still only because the host was the voice stack.
fn respond_voice_stack(
    lane: &mut PhoneLane,
    job: &TurnJob,
    persona: &'static str,
    in_ms: u32,
    t0: Instant,
    wav_in: &[u8],
    ask: &Mutex<Option<ResearchAsk>>,
) -> Option<TurnReply> {
    use galion_cascade::research_handoff::{research_cue, RESEARCH_GREETING, RESEARCH_VOICE};
    let (transcript, asr) = match lane.hear_wav(wav_in) {
        Ok(heard) => heard,
        Err(PhoneError::Superseded) => return None,
        Err(e) => {
            return Some(lane_error(LaneFail {
                id: job.id,
                persona,
                in_ms,
                t0,
                err: e,
                text: String::new(),
                asr: "none",
                llm: "none",
            }))
        }
    };
    let Some(cue) = research_cue(&transcript) else {
        return pack_phone(
            job.id,
            persona,
            in_ms,
            t0,
            lane.speak_heard(transcript, asr),
        );
    };
    let llm = if cue.question.is_empty() {
        "research"
    } else {
        "hermes"
    };
    let mut spoken = if cue.question.is_empty() {
        RESEARCH_GREETING.to_string()
    } else {
        ask_research(ask, &cue.question)
    };
    if galion_cascade::contains_forbidden(&spoken) {
        spoken = galion_cascade::REFUSED.to_string();
    }
    let mut result = lane.speak_fixed(&spoken);
    if let Ok(turn) = result.as_mut() {
        turn.transcript = transcript.clone();
        turn.asr = asr;
        turn.llm = llm;
        turn.reply_text = spoken.clone();
    }
    let mut reply = match result {
        Ok(turn) => pack_ok(job.id, RESEARCH_VOICE, in_ms, t0, turn),
        Err(PhoneError::Superseded) => return None,
        Err(e) => lane_error(LaneFail {
            id: job.id,
            persona: RESEARCH_VOICE,
            in_ms,
            t0,
            err: e,
            text: spoken,
            asr: asr.id(),
            llm,
        }),
    };
    if reply.error.is_none() {
        reply.stages = format!("research=handoff {}", reply.stages);
    } else {
        reply.transcript = transcript;
        reply.llm = llm;
        reply.stages = "research=handoff".into();
    }
    Some(reply)
}

fn pack_phone(
    id: u64,
    persona: &'static str,
    in_ms: u32,
    t0: Instant,
    result: Result<galion_cascade::PhoneTurn, PhoneError>,
) -> Option<TurnReply> {
    match result {
        Ok(turn) => Some(pack_ok(id, persona, in_ms, t0, turn)),
        Err(PhoneError::Superseded) => None,
        Err(e) => Some(lane_error(LaneFail {
            id,
            persona,
            in_ms,
            t0,
            err: e,
            text: String::new(),
            asr: "none",
            llm: "none",
        })),
    }
}

fn pack_ok(
    id: u64,
    persona: &'static str,
    in_ms: u32,
    t0: Instant,
    mut t: galion_cascade::PhoneTurn,
) -> TurnReply {
    t.stamps.turn_id = id;
    let pcm = reply_pcm(&t.wav_bytes);
    let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
    TurnReply {
        id,
        transcript: t.transcript,
        text: t.reply_text,
        pcm,
        wav: t.wav_bytes,
        ms,
        backend: t.speech_backend,
        asr: t.asr.id(),
        llm: t.llm,
        stages: stage_log(
            &format!(
                "asr={} llm={} tts={} tts_first={} first_audio={}",
                t.stage_ms.asr, t.stage_ms.llm, t.stage_ms.tts, t.stage_ms.tts_first_pcm, t.stage_ms.first_audio_ms
            ),
            &t.stamps,
        ),
        error: None,
        in_ms,
        took_ms: t0.elapsed().as_millis() as u32,
        persona,
    }
}

struct LaneFail {
    id: u64,
    persona: &'static str,
    in_ms: u32,
    t0: Instant,
    err: PhoneError,
    text: String,
    asr: &'static str,
    llm: &'static str,
}

fn lane_error(fail: LaneFail) -> TurnReply {
    let LaneFail {
        id,
        persona,
        in_ms,
        t0,
        err: e,
        text,
        asr,
        llm,
    } = fail;
    let kind = match &e {
        PhoneError::AsrUnratified(_) => "AsrUnratified (set GALION_ASR=vosk and place the small-en model; docs/asr/VOSK.md)",
        PhoneError::AsrUnavailable(_) => "AsrUnavailable (Vosk model or worker missing; typed text still works; no transcript invented)",
        PhoneError::NoSpeech(_) => "NoSpeech (GALION_SPEECH / Piper weights)",
        PhoneError::BadWav(_) => "BadWav",
        PhoneError::Empty => "Empty",
        _ => "Cascade",
    };
    let pcm = tone();
    let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
    TurnReply {
        id,
        transcript: String::new(),
        text,
        wav: wav_bytes(&pcm, LANE_RATE_HZ),
        pcm,
        ms,
        backend: "tone",
        asr,
        llm,
        stages: String::new(),
        error: Some(format!("{kind}: {e}")),
        in_ms,
        took_ms: t0.elapsed().as_millis() as u32,
        persona,
    }
}

/// Turn boundary, not the 80 ms tick. Runs only when `GALION_OPENMIND_SEAM`
/// is on. A missing store or weights file keeps the lane reply. A memory
/// hit speaks the exact stored line. Hermes is not called from here.
fn respond_seamed(
    lane: &mut PhoneLane,
    mut job: TurnJob,
    persona: &'static str,
    in_ms: u32,
    t0: Instant,
    wav_in: &mut Vec<u8>,
) -> Option<TurnReply> {
    let id = job.id;
    let heard = lane.hear_wav(wav_in);
    for b in wav_in.iter_mut() {
        *b = 0;
    }
    std::hint::black_box(wav_in.as_slice());
    job.wipe();
    let (transcript, asr) = match heard {
        Ok(v) => v,
        Err(PhoneError::Superseded) => return None,
        Err(e) => {
            return Some(lane_error(LaneFail {
                id,
                persona,
                in_ms,
                t0,
                err: e,
                text: String::new(),
                asr: "none",
                llm: "none",
            }))
        }
    };
    let asr_label = asr.id();
    let seam = TurnSeam::from_env();
    let _ = seam.note_turn(id, "user", &transcript);
    let decision = seam.consult(id, matches!(lane.cfg.llm, LlmMode::Hermes));
    let token = decision.stage_token();
    let result = match decision.reply_from {
        ReplyFrom::Stored { text, .. } => {
            let mut result = lane.speak_fixed(&text);
            if let Ok(turn) = result.as_mut() {
                turn.transcript = transcript;
                turn.asr = asr;
                turn.llm = "local-store";
                turn.reply_text = text;
            }
            result
        }
        ReplyFrom::Lane => lane.speak_heard(transcript, asr),
    };
    match result {
        Ok(turn) => {
            let mut reply = pack_ok(id, persona, in_ms, t0, turn);
            reply.stages = format!("{} {}", reply.stages, token);
            Some(reply)
        }
        Err(PhoneError::Superseded) => None,
        Err(e) => {
            let mut reply = lane_error(LaneFail {
                id,
                persona,
                in_ms,
                t0,
                err: e,
                text: String::new(),
                asr: asr_label,
                llm: "none",
            });
            reply.stages = token;
            Some(reply)
        }
    }
}

fn respond(
    lane: &mut PhoneLane,
    mut job: TurnJob,
    gate: &BusGate,
    prosody: &ProsodySlot,
    ask: &Mutex<Option<ResearchAsk>>,
) -> Option<TurnReply> {
    lane.set_stamp_turn(job.id);
    let persona = gate.persona_id().as_str();
    lane.set_delivery(persona, prosody.load());
    lane.arm_cancel(gate.cancel_token(), job.id);
    let t0 = Instant::now();
    let in_ms = (job.pcm.len() as u64 * 1000 / job.rate.max(1) as u64) as u32;
    let mut wav_in = wav_bytes(&job.pcm, job.rate);
    if job.voice_stack {
        let voice = respond_voice_stack(lane, &job, persona, in_ms, t0, &wav_in, ask);
        for b in &mut wav_in {
            *b = 0;
        }
        std::hint::black_box(wav_in.as_slice());
        job.wipe();
        return voice;
    }
    if seam_enabled_from_env() {
        return respond_seamed(lane, job, persona, in_ms, t0, &mut wav_in);
    }
    let result = lane.turn_wav(&wav_in);
    for b in &mut wav_in {
        *b = 0;
    }
    std::hint::black_box(wav_in.as_slice());
    job.wipe();
    match result {
        Ok(t) => Some(pack_ok(job.id, persona, in_ms, t0, t)),
        Err(PhoneError::Superseded) => None,
        Err(e) => {
            let kind = match &e {
                PhoneError::AsrUnratified(_) => "AsrUnratified (set GALION_ASR=vosk and place the small-en model; docs/asr/VOSK.md)",
                PhoneError::AsrUnavailable(_) => "AsrUnavailable (Vosk model or worker missing; typed text still works; no transcript invented)",
                PhoneError::NoSpeech(_) => "NoSpeech (GALION_SPEECH / Piper weights)",
                PhoneError::BadWav(_) => "BadWav",
                PhoneError::Empty => "Empty",
                _ => "Cascade",
            };
            let pcm = tone();
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id: job.id,
                transcript: String::new(),
                text: String::new(),
                wav: wav_bytes(&pcm, LANE_RATE_HZ),
                pcm,
                ms,
                backend: "tone",
                asr: "none",
                llm: "none",
                stages: String::new(),
                error: Some(format!("{kind}: {e}")),
                in_ms,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
    }
}

/// 24 kHz mono samples of a PCM16 WAV (data chunk scan; empty on malformed input).
fn reply_pcm(b: &[u8]) -> Vec<i16> {
    let mut i = 12;
    while i + 8 <= b.len() {
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        if &b[i..i + 4] == b"data" {
            let d = &b[i + 8..(i + 8 + n).min(b.len())];
            return d
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
        }
        i += 8 + n + (n & 1);
    }
    Vec::new()
}

/// Clear two-tone "ding-dong" (660 Hz then 440 Hz, 250 ms each, 10 ms ramps) @ 24 kHz.
pub fn tone() -> Vec<i16> {
    let sr = LANE_RATE_HZ as usize;
    let seg = sr / 4;
    let ramp = sr / 100;
    let mut out = Vec::with_capacity(seg * 2 + sr / 10);
    for f in [660.0f32, 440.0] {
        for i in 0..seg {
            let env = (i.min(seg - 1 - i) as f32 / ramp as f32).min(1.0);
            let ph = std::f32::consts::TAU * f * i as f32 / sr as f32;
            out.push((0.35 * env * ph.sin() * i16::MAX as f32) as i16);
        }
    }
    out.extend(std::iter::repeat(0).take(sr / 10));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, MutexGuard};

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    struct EnvGuard { saved: Vec<(String, Option<String>)> }
    impl EnvGuard {
        fn set(pairs: &[(&str, &str)]) -> Self {
            let saved = pairs.iter().map(|(k, v)| {
                let prev = std::env::var(k).ok();
                std::env::set_var(k, v);
                ((*k).to_string(), prev)
            }).collect();
            Self { saved }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, prev) in self.saved.drain(..) {
                match prev { Some(v) => std::env::set_var(k, v), None => std::env::remove_var(k) }
            }
        }
    }

    #[test]
    fn tone_is_audible_and_bounded() {
        let t = tone();
        assert_eq!(t.len(), 12_000 + 2_400);
        assert!(t.iter().any(|&s| s.unsigned_abs() > 10_000));
    }
    #[test]
    fn stale_reply_dropped_after_supersede() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]); // -> AsrUnratified -> tone fallback
        let w = TurnWorker::spawn();
        assert!(w
            .try_submit(TurnJob {
                id: 1,
                rate: 16_000,
                pcm: vec![0; 160],
                voice_stack: false,
            })
            .is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 5 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let r = got.expect("reply");
        assert_eq!((r.id, r.backend, r.persona), (1, "tone", "samantha"));
        assert!(r
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with("AsrUnratified")));
        assert_eq!(reply_pcm(&r.wav), r.pcm);
        assert!(r.wav.starts_with(b"RIFF"));
        w.supersede(9); // barge-in: anything older than 9 is stale
        assert_eq!(w.bus().cancel(), 9);
        assert!(w.try_recv().is_none());
    }

    #[test]
    fn explicit_jarvis_is_on_the_reply_and_survives_barge_in() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]);
        let w = TurnWorker::spawn();
        w.bus()
            .try_send(crate::bus::BusMsg::persona_explicit(w.bus().alloc_id(), 0, crate::bus::PersonaId::Jarvis))
            .unwrap();
        assert!(w.try_submit(TurnJob { id: 1, rate: 16_000, pcm: vec![0; 160], voice_stack: false }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 5 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.expect("reply").persona, "jarvis");
        w.supersede(9);
        assert_eq!(w.bus().persona_id(), crate::bus::PersonaId::Jarvis);
    }

    #[test]
    fn older_submit_does_not_rewind_cancel_and_is_dropped_before_asr() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]);
        let w = TurnWorker::spawn();
        w.supersede(9);
        assert!(w
            .try_submit(TurnJob {
                id: 3,
                rate: 16_000,
                pcm: vec![1, 0],
                voice_stack: false,
            })
            .is_ok());
        assert_eq!(
            w.bus().cancel(),
            9,
            "an older submit must not rewind cancel"
        );
        let t0 = Instant::now();
        while w.bus().dropped_before_asr() == 0 && t0.elapsed().as_secs() < 5 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            w.bus().dropped_before_asr() >= 1,
            "turn 3 was not dropped before ASR"
        );
        assert!(w.try_recv().is_none());
    }

    #[test]
    fn reply_and_job_pcm_are_zeroised() {
        let mut job = TurnJob { id: 1, rate: 16_000, pcm: vec![9, -9, 12345], voice_stack: false };
        job.wipe();
        assert!(job.pcm.iter().all(|&s| s == 0));
        let mut reply = TurnReply {
            id: 1,
            transcript: "SECRET_TRANSCRIPT".into(),
            text: "hello".into(),
            pcm: vec![42, -42],
            wav: b"RIFFPCM".to_vec(),
            ms: 1,
            backend: "tone",
            asr: "none",
            llm: "none",
            stages: String::new(),
            error: None,
            persona: "samantha",
            in_ms: 1,
            took_ms: 1,
        };
        reply.wipe();
        assert!(reply.pcm.iter().all(|&s| s == 0));
        assert!(reply.wav.iter().all(|&b| b == 0));
        assert!(reply.transcript.is_empty() && reply.text.is_empty());
    }

    #[test]
    fn speech_off_publishes_tone_and_error_label() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "stub")]);
        let w = TurnWorker::spawn();
        assert!(w.try_submit(TurnJob { id: 3, rate: 16_000, pcm: galion_cascade::stub_fixture_pcm16("hello"), voice_stack: false }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 8 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let r = got.expect("tone reply");
        assert_eq!((r.id, r.backend), (3, "tone"));
        let err = r.error.as_deref().unwrap_or("");
        assert!(err.contains("NoSpeech"), "{err}");
        assert!(err.contains("GALION_SPEECH=off"), "{err}");
        assert!(r.wav.starts_with(b"RIFF"));
        assert!(r.pcm.iter().any(|&s| s.unsigned_abs() > 1000));
    }

    fn wait_reply(w: &TurnWorker) -> TurnReply {
        let t0 = Instant::now();
        loop {
            if let Some(r) = w.try_recv() {
                return r;
            }
            assert!(t0.elapsed().as_secs() < 8, "reply did not arrive");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn voice_stack_hands_the_spoken_question_to_a_stub_hermes() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "stub")]);
        let seen = Arc::new(Mutex::new(String::new()));
        let seen_w = Arc::clone(&seen);
        let w = TurnWorker::spawn();
        w.set_research_ask(Arc::new(move |q| {
            *seen_w.lock().unwrap() = q.to_string();
            "The wash stays near-white.".into()
        }));
        let pcm = galion_cascade::stub_fixture_pcm16("go deep what colour is the wash");
        assert!(w.try_submit(TurnJob { id: 4, rate: 16_000, pcm, voice_stack: true }).is_ok());
        let r = wait_reply(&w);
        assert_eq!(seen.lock().unwrap().as_str(), "what colour is the wash");
        assert_eq!(r.persona, galion_cascade::research_handoff::RESEARCH_VOICE);
        assert_eq!(r.text, "The wash stays near-white.");
        assert_eq!(r.transcript, "go deep what colour is the wash");
        assert_eq!(r.llm, "hermes");
        assert!(r.stages.contains("research=handoff"), "{}", r.stages);
        assert!(!r.text.contains("Bearer"));
    }

    #[test]
    fn voice_stack_cue_without_a_question_greets_and_does_not_ask() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "stub")]);
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let asked_w = Arc::clone(&asked);
        let w = TurnWorker::spawn();
        w.set_research_ask(Arc::new(move |_| {
            asked_w.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            "should not be spoken".into()
        }));
        let pcm = galion_cascade::stub_fixture_pcm16("please go deep");
        assert!(w.try_submit(TurnJob { id: 5, rate: 16_000, pcm, voice_stack: true }).is_ok());
        let r = wait_reply(&w);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(r.text, galion_cascade::research_handoff::RESEARCH_GREETING);
        assert_eq!(r.persona, galion_cascade::research_handoff::RESEARCH_VOICE);
        assert_eq!(r.llm, "research");
    }

    #[test]
    fn one_page_host_does_not_hand_the_same_words_to_hermes() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "stub")]);
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let asked_w = Arc::clone(&asked);
        let w = TurnWorker::spawn();
        w.set_research_ask(Arc::new(move |_| {
            asked_w.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            "The wash stays near-white.".into()
        }));
        let pcm = galion_cascade::stub_fixture_pcm16("research this why the fill is locked");
        assert!(w.try_submit(TurnJob { id: 6, rate: 16_000, pcm, voice_stack: false }).is_ok());
        let r = wait_reply(&w);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(r.persona, "samantha");
        assert_ne!(r.text, "The wash stays near-white.");
        assert!(r.text.is_empty(), "speech is off, so the ordinary turn keeps the tone and no research line");
    }

    #[test]
    fn openmind_seam_blocks_when_the_store_is_missing_and_witnesses_when_it_exists() {
        let _lock = env_lock();
        let dir = std::env::temp_dir().join(format!(
            "galion-seam-worker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _env = EnvGuard::set(&[
            ("GALION_SPEECH", "off"),
            ("GALION_ASR", "stub"),
            ("GALION_LLM", "canned"),
            ("GALION_OPENMIND_SEAM", "1"),
            ("GALION_OPENMIND_REMEMBER", "yes"),
            ("GALION_STUDENT_GQ", ""),
            ("GALION_OPENMIND_STORE", ""),
        ]);
        let w = TurnWorker::spawn();
        assert!(w.try_submit(TurnJob {
            id: 21,
            rate: 16_000,
            pcm: galion_cascade::stub_fixture_pcm16("hello"),
            voice_stack: false,
        }).is_ok());
        let blocked = wait_reply(&w);
        assert!(blocked.stages.contains("seam=blocked:store+weights"), "{}", blocked.stages);
        assert!(blocked.stages.contains("memory=blocked:store"), "{}", blocked.stages);
        assert!(blocked.stages.contains("fallback=canned"), "{}", blocked.stages);
        assert_ne!(blocked.llm, "local-store");
        assert!(blocked.text.is_empty(), "no store, so no memory line was spoken");
        assert!(!blocked.stages.contains("hello"));
        drop(w);
        drop(_env);

        let _env = EnvGuard::set(&[
            ("GALION_SPEECH", "off"),
            ("GALION_ASR", "stub"),
            ("GALION_LLM", "canned"),
            ("GALION_OPENMIND_SEAM", "1"),
            ("GALION_OPENMIND_REMEMBER", "yes"),
            ("GALION_STUDENT_GQ", ""),
            ("GALION_OPENMIND_STORE", dir.to_str().unwrap()),
        ]);
        let w = TurnWorker::spawn();
        assert!(w.try_submit(TurnJob {
            id: 1,
            rate: 16_000,
            pcm: galion_cascade::stub_fixture_pcm16("the lamp is on"),
            voice_stack: false,
        }).is_ok());
        let first = wait_reply(&w);
        assert!(first.stages.contains("seam=blocked:weights"), "{}", first.stages);
        assert!(first.stages.contains("memory=empty"), "{}", first.stages);
        assert!(first.text.is_empty());
        assert!(w.try_submit(TurnJob {
            id: 2,
            rate: 16_000,
            pcm: galion_cascade::stub_fixture_pcm16("hello"),
            voice_stack: false,
        }).is_ok());
        let later = wait_reply(&w);
        assert!(later.stages.contains("memory=hit:1"), "{}", later.stages);
        assert!(later.stages.contains("seam=blocked:weights"), "{}", later.stages);
        assert!(!later.stages.contains("the lamp is on"));
        assert_ne!(later.llm, "local-store");
        let stored = std::fs::read_to_string(dir.join("turns.jsonl")).unwrap();
        assert_eq!(stored, "v1\t1\tuser\tthe lamp is on\nv1\t2\tuser\thello\n");
        assert!(later.error.as_deref().is_some_and(|e| e.contains("NoSpeech")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
