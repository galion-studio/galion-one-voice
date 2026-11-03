// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2-S1 process bus (`docs/galion-voice-v2/IPC.md`).
//!
//! One typed [`BusMsg`] envelope for capture, the turn worker, reply publish,
//! the orb, and the consent gate. The lanes sit next to [`crate::turn_worker`]
//! and the existing `MicTap` / `TurnInbox` / `ReplyBox`. This is not a broker:
//! no extra crate, no socket, no second HTTP server.
//!
//! Every send is `try_send`. A full lane drops or coalesces. Nothing here waits
//! on ASR, a reply model, or TTS, and nothing here runs inside `Heartbeat::step`.
//! Barge-in stores a cancel token. Receivers drop `turn_id < cancel` before ASR
//! and again before a reply is published. The worker thread is not killed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

/// Mic levels kept. Latest RMS replaces the previous sample.
pub const MIC_DEPTH: usize = 1;
/// Finished utterances. Matches `TurnInbox`'s `MAX_PENDING_TURNS`.
pub const UTTERANCE_DEPTH: usize = 8;
/// Published `SpeechPcm` / `ReplyText`. Matches `ReplyBox`'s `MAX_STORED_REPLIES`.
pub const REPLY_DEPTH: usize = 8;
/// Partials are turn-scoped text. Same cap as utterances; oldest is dropped.
pub const PARTIAL_DEPTH: usize = 8;
/// Faults carry no audio. Oldest is dropped.
pub const FAULT_DEPTH: usize = 8;
/// Worker job slot. `try_send` returns the message when it is already taken.
pub const JOB_DEPTH: usize = 1;
/// Worker → heartbeat replies. A full lane drops the new reply.
pub const WORKER_REPLY_DEPTH: usize = 4;
/// Utterance pcm cap. Matches `MAX_BODY_BYTES` (4 MiB). No disk spill.
pub const MAX_UTTERANCE_BYTES: usize = 4 * 1024 * 1024;
/// `SpeechPcm` rate. The phone lane speaks at 24 kHz.
pub const SPEECH_RATE_HZ: u32 = 24_000;

/// `PartialAsr.source` for a decoded stub fixture.
pub const ASR_STUB_FIXTURE: &str = "stub-fixture";
/// `PartialAsr.source` for the length-bucket placeholder. Not recognition.
pub const ASR_INTERIM_PLACEHOLDER: &str = "interim-placeholder";

/// EmotionWire consent bit 0 (`sense`). Reply emotion tags need it. Default deny.
pub const CONSENT_SENSE: u8 = 1 << 0;
/// No purpose recorded. Fresh consent is deny.
pub const PURPOSE_NONE: &str = "none";
/// Personal / consumer conversation. Still deny until the bit is granted.
pub const PURPOSE_PERSONAL_ASSISTANT: &str = "personal-assistant";

/// `true` for the two labels that must never be shown as recognition.
pub fn asr_source_is_placeholder(source: &str) -> bool {
    matches!(source, ASR_STUB_FIXTURE | ASR_INTERIM_PLACEHOLDER)
}

/// Orb modes from `IPC.md`. Same names as `galion_render::OrbMotion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbMode {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Offline,
}

impl OrbMode {
    pub fn as_str(self) -> &'static str {
        match self {
            OrbMode::Idle => "idle",
            OrbMode::Listening => "listening",
            OrbMode::Thinking => "thinking",
            OrbMode::Speaking => "speaking",
            OrbMode::Offline => "offline",
        }
    }
}

impl From<galion_render::OrbMotion> for OrbMode {
    fn from(m: galion_render::OrbMotion) -> Self {
        match m {
            galion_render::OrbMotion::Idle => OrbMode::Idle,
            galion_render::OrbMotion::Listening => OrbMode::Listening,
            galion_render::OrbMotion::Thinking => OrbMode::Thinking,
            galion_render::OrbMotion::Speaking => OrbMode::Speaking,
            galion_render::OrbMotion::Offline => OrbMode::Offline,
        }
    }
}

/// What a full or rejected `try_send` did. The caller never blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendOutcome {
    /// Stored. The lane had room.
    Queued,
    /// Mic / orb / consent slot already held a sample. The new one replaced it.
    Replaced,
    /// A capped FIFO dropped its oldest message to make room.
    EvictedOldest,
    /// `BargeIn` updated the cancel token. Nothing was queued.
    Applied,
    /// Not stored. Stale, or the worker-reply lane was full.
    Dropped,
}

/// `try_send` refused the message. The caller still holds it.
#[derive(Debug)]
pub enum SendError {
    /// Job lane already holds a message (depth 1), or the worker is gone.
    Full(BusMsg),
    /// Utterance pcm is larger than [`MAX_UTTERANCE_BYTES`].
    TooLarge(BusMsg),
}

/// One hop on the bus. `t_ms` is heartbeat time (frame × 80), not wall clock.
#[derive(Clone, Debug, PartialEq)]
pub struct BusMsg {
    /// Monotonic per sender. [`ProcessBus::alloc_id`] starts at 1.
    pub id: u64,
    /// Utterance this belongs to. `0` is not an utterance (mic, consent, orb).
    pub turn_id: u64,
    /// Heartbeat time in milliseconds.
    pub t_ms: u64,
    /// Cancel token stamped by the sender. Drop when `turn_id < cancel`.
    pub cancel: u64,
    pub body: BusBody,
}

/// Message bodies from `IPC.md`. `PartialAsr.is_final` is the spec's `final` flag.
#[derive(Clone, Debug, PartialEq)]
pub enum BusBody {
    MicLevel {
        rms: f32,
    },
    Utterance {
        rate: u32,
        pcm: Vec<u8>,
    },
    PartialAsr {
        text: String,
        source: &'static str,
        is_final: bool,
    },
    /// `emotion` is `None` unless consent `sense` is granted.
    ReplyText {
        text: String,
        emotion: Option<String>,
    },
    SpeechPcm {
        rate: u32,
        pcm: Vec<u8>,
        ms: u32,
    },
    OrbState {
        mode: OrbMode,
        level: f32,
        env: f32,
    },
    Consent {
        bits: u8,
        purpose: &'static str,
    },
    BargeIn {
        turn_id: u64,
    },
    Fault {
        stage: String,
        kind: String,
    },
}

impl BusMsg {
    /// `true` when this utterance is older than `cancel` (or than the token
    /// stamped on the message, whichever is later). `turn_id == 0` is kept.
    pub fn is_stale(&self, cancel: u64) -> bool {
        let cancel = cancel.max(self.cancel);
        self.turn_id != 0 && self.turn_id < cancel
    }

    pub fn mic_level(id: u64, t_ms: u64, rms: f32) -> Self {
        let rms = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            id,
            turn_id: 0,
            t_ms,
            cancel: 0,
            body: BusBody::MicLevel { rms },
        }
    }

    pub fn utterance(
        id: u64,
        turn_id: u64,
        t_ms: u64,
        cancel: u64,
        rate: u32,
        pcm: Vec<u8>,
    ) -> Self {
        Self {
            id,
            turn_id,
            t_ms,
            cancel,
            body: BusBody::Utterance { rate, pcm },
        }
    }

    pub fn partial_asr(
        id: u64,
        turn_id: u64,
        t_ms: u64,
        cancel: u64,
        text: impl Into<String>,
        source: &'static str,
        is_final: bool,
    ) -> Self {
        Self {
            id,
            turn_id,
            t_ms,
            cancel,
            body: BusBody::PartialAsr {
                text: text.into(),
                source,
                is_final,
            },
        }
    }

    /// Emotion is stored only when `sense_granted` is set. Otherwise the tag is dropped.
    pub fn reply_text(
        id: u64,
        turn_id: u64,
        t_ms: u64,
        cancel: u64,
        text: impl Into<String>,
        emotion: Option<String>,
        sense_granted: bool,
    ) -> Self {
        let emotion = if sense_granted {
            emotion.filter(|s| !s.is_empty())
        } else {
            None
        };
        Self {
            id,
            turn_id,
            t_ms,
            cancel,
            body: BusBody::ReplyText {
                text: text.into(),
                emotion,
            },
        }
    }

    pub fn speech_pcm(
        id: u64,
        turn_id: u64,
        t_ms: u64,
        cancel: u64,
        pcm: Vec<u8>,
        ms: u32,
    ) -> Self {
        Self {
            id,
            turn_id,
            t_ms,
            cancel,
            body: BusBody::SpeechPcm {
                rate: SPEECH_RATE_HZ,
                pcm,
                ms,
            },
        }
    }

    pub fn orb_state(id: u64, t_ms: u64, mode: OrbMode, level: f32, env: f32) -> Self {
        let level = if level.is_finite() {
            level.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let env = if env.is_finite() {
            env.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            id,
            turn_id: 0,
            t_ms,
            cancel: 0,
            body: BusBody::OrbState { mode, level, env },
        }
    }

    pub fn consent(id: u64, t_ms: u64, bits: u8, purpose: &'static str) -> Self {
        Self {
            id,
            turn_id: 0,
            t_ms,
            cancel: 0,
            body: BusBody::Consent { bits, purpose },
        }
    }

    /// Deny-default consent: no bits, no purpose.
    pub fn consent_deny(id: u64, t_ms: u64) -> Self {
        Self::consent(id, t_ms, 0, PURPOSE_NONE)
    }

    pub fn barge_in(id: u64, turn_id: u64, t_ms: u64) -> Self {
        Self {
            id,
            turn_id,
            t_ms,
            cancel: turn_id,
            body: BusBody::BargeIn { turn_id },
        }
    }

    /// `kind` is a short error class. Do not put pcm or a transcript here.
    pub fn fault(
        id: u64,
        turn_id: u64,
        t_ms: u64,
        cancel: u64,
        stage: impl Into<String>,
        kind: impl Into<String>,
    ) -> Self {
        Self {
            id,
            turn_id,
            t_ms,
            cancel,
            body: BusBody::Fault {
                stage: stage.into(),
                kind: kind.into(),
            },
        }
    }
}

/// Little-endian PCM16. The utterance and speech bodies carry bytes, not `i16`.
pub fn pcm16_to_bytes(pcm: &[i16]) -> Vec<u8> {
    let mut v = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

pub fn bytes_to_pcm16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// Where a two-stage pass stopped. ASR runs only after the first gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageResult {
    DroppedBeforeAsr,
    DroppedBeforePublish,
    Published,
}

struct Inner {
    /// Shared with Piper. A stale `turn_id` drops clauses; the process stays up.
    cancel: Arc<AtomicU64>,
    ids: AtomicU64,
    dropped_before_asr: AtomicU64,
    dropped_before_publish: AtomicU64,
    worker_replies_dropped: AtomicU64,
    worker_reply_queued: AtomicUsize,
    mic: Mutex<Option<BusMsg>>,
    orb: Mutex<Option<BusMsg>>,
    consent: Mutex<Option<BusMsg>>,
    utterances: Mutex<VecDeque<BusMsg>>,
    partials: Mutex<VecDeque<BusMsg>>,
    replies: Mutex<VecDeque<BusMsg>>,
    faults: Mutex<VecDeque<BusMsg>>,
    job_rx: Mutex<Option<Receiver<BusMsg>>>,
    /// Held so the worker-reply `sync_channel` stays connected for `try_send`.
    #[allow(dead_code)]
    worker_reply_rx: Mutex<Option<Receiver<BusMsg>>>,
}

/// Cancel token and drop counters, without the job sender.
///
/// The worker thread holds this. It does not hold a [`ProcessBus`] clone, so
/// dropping the host disconnects the job `recv` and the thread exits.
#[derive(Clone)]
pub(crate) struct BusGate {
    inner: Arc<Inner>,
}

impl BusGate {
    pub(crate) fn admit_asr(&self, turn_id: u64) -> bool {
        admit(&self.inner, turn_id, false)
    }

    pub(crate) fn admit_publish(&self, turn_id: u64) -> bool {
        admit(&self.inner, turn_id, true)
    }

    pub(crate) fn note_worker_reply_dropped(&self) {
        self.inner
            .worker_replies_dropped
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Same atomic [`ProcessBus::supersede`] writes. Piper reads it to drop clauses.
    pub(crate) fn cancel_token(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.inner.cancel)
    }
}

fn admit(inner: &Inner, turn_id: u64, publish: bool) -> bool {
    let cancel = inner.cancel.load(Ordering::Acquire);
    if turn_id != 0 && turn_id < cancel {
        let counter = if publish {
            &inner.dropped_before_publish
        } else {
            &inner.dropped_before_asr
        };
        counter.fetch_add(1, Ordering::Relaxed);
        false
    } else {
        true
    }
}

/// In-process lanes. Clone shares the queues and the cancel token.
///
/// Senders live on this struct, not in the shared arc. The worker thread must
/// not clone [`ProcessBus`] (use [`ProcessBus::gate`]).
#[derive(Clone)]
pub struct ProcessBus {
    inner: Arc<Inner>,
    job_tx: SyncSender<BusMsg>,
    worker_reply_tx: SyncSender<BusMsg>,
}

impl ProcessBus {
    pub fn new() -> Self {
        let (job_tx, job_rx) = sync_channel(JOB_DEPTH);
        let (worker_reply_tx, worker_reply_rx) = sync_channel(WORKER_REPLY_DEPTH);
        Self {
            inner: Arc::new(Inner {
                cancel: Arc::new(AtomicU64::new(0)),
                ids: AtomicU64::new(0),
                dropped_before_asr: AtomicU64::new(0),
                dropped_before_publish: AtomicU64::new(0),
                worker_replies_dropped: AtomicU64::new(0),
                worker_reply_queued: AtomicUsize::new(0),
                mic: Mutex::new(None),
                orb: Mutex::new(None),
                consent: Mutex::new(None),
                utterances: Mutex::new(VecDeque::new()),
                partials: Mutex::new(VecDeque::new()),
                replies: Mutex::new(VecDeque::new()),
                faults: Mutex::new(VecDeque::new()),
                job_rx: Mutex::new(Some(job_rx)),
                worker_reply_rx: Mutex::new(Some(worker_reply_rx)),
            }),
            job_tx,
            worker_reply_tx,
        }
    }

    pub(crate) fn gate(&self) -> BusGate {
        BusGate {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Take the job receiver once. [`crate::turn_worker::TurnWorker`] owns it.
    pub(crate) fn take_job_rx(&self) -> Option<Receiver<BusMsg>> {
        lock(&self.inner.job_rx).take()
    }

    pub fn alloc_id(&self) -> u64 {
        self.inner.ids.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn cancel(&self) -> u64 {
        self.inner.cancel.load(Ordering::Acquire)
    }

    /// `false` when `turn_id` is older than cancel. Counts a drop before publish.
    pub(crate) fn admit_publish(&self, turn_id: u64) -> bool {
        admit(&self.inner, turn_id, true)
    }

    /// High-water barge-in token. Does not move backwards. Does not kill the worker.
    pub fn supersede(&self, turn_id: u64) {
        self.raise_cancel(turn_id);
    }

    pub fn dropped_before_asr(&self) -> u64 {
        self.inner.dropped_before_asr.load(Ordering::Relaxed)
    }

    pub fn dropped_before_publish(&self) -> u64 {
        self.inner.dropped_before_publish.load(Ordering::Relaxed)
    }

    pub fn worker_replies_dropped(&self) -> u64 {
        self.inner.worker_replies_dropped.load(Ordering::Relaxed)
    }

    pub fn worker_reply_queued(&self) -> usize {
        self.inner.worker_reply_queued.load(Ordering::Relaxed)
    }

    pub fn utterance_len(&self) -> usize {
        lock(&self.inner.utterances).len()
    }

    pub fn reply_len(&self) -> usize {
        lock(&self.inner.replies).len()
    }

    pub fn mic_len(&self) -> usize {
        usize::from(lock(&self.inner.mic).is_some())
    }

    /// Route on `body`. Utterances go to the depth-8 lane, not the job slot.
    /// The job slot is [`Self::try_send_job`] (depth 1).
    pub fn try_send(&self, msg: BusMsg) -> Result<SendOutcome, SendError> {
        if let BusBody::Utterance { pcm, .. } = &msg.body {
            if pcm.len() > MAX_UTTERANCE_BYTES {
                return Err(SendError::TooLarge(msg));
            }
        }
        self.observe_cancel(&msg);
        Ok(match msg.body {
            BusBody::MicLevel { .. } => self.enqueue_slot(&self.inner.mic, msg),
            BusBody::OrbState { .. } => self.enqueue_slot(&self.inner.orb, msg),
            BusBody::Consent { .. } => self.enqueue_slot(&self.inner.consent, msg),
            BusBody::Utterance { .. } => {
                self.enqueue_fifo(&self.inner.utterances, UTTERANCE_DEPTH, msg, Stale::Asr)
            }
            BusBody::PartialAsr { .. } => {
                self.enqueue_fifo(&self.inner.partials, PARTIAL_DEPTH, msg, Stale::Publish)
            }
            BusBody::ReplyText { .. } | BusBody::SpeechPcm { .. } => {
                self.enqueue_fifo(&self.inner.replies, REPLY_DEPTH, msg, Stale::Publish)
            }
            BusBody::Fault { .. } => {
                self.enqueue_fifo(&self.inner.faults, FAULT_DEPTH, msg, Stale::Quiet)
            }
            BusBody::BargeIn { .. } => SendOutcome::Applied,
        })
    }

    /// Depth 1. `Err` returns the message so the host can keep one pending job.
    pub fn try_send_job(&self, msg: BusMsg) -> Result<(), BusMsg> {
        self.observe_cancel(&msg);
        match self.job_tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(m) | TrySendError::Disconnected(m)) => Err(m),
        }
    }

    /// Depth 4. A full lane drops the new reply and returns [`SendOutcome::Dropped`].
    pub fn try_send_worker_reply(&self, msg: BusMsg) -> SendOutcome {
        self.observe_cancel(&msg);
        if msg.is_stale(self.cancel()) {
            self.inner
                .dropped_before_publish
                .fetch_add(1, Ordering::Relaxed);
            return SendOutcome::Dropped;
        }
        match self.worker_reply_tx.try_send(msg) {
            Ok(()) => {
                self.inner
                    .worker_reply_queued
                    .fetch_add(1, Ordering::Relaxed);
                SendOutcome::Queued
            }
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.inner
                    .worker_replies_dropped
                    .fetch_add(1, Ordering::Relaxed);
                SendOutcome::Dropped
            }
        }
    }

    /// Publish path for `ReplyText` / `SpeechPcm` (depth 8, evict oldest).
    pub fn try_send_reply(&self, msg: BusMsg) -> SendOutcome {
        match self.try_send(msg) {
            Ok(outcome) => outcome,
            Err(SendError::Full(msg) | SendError::TooLarge(msg)) => {
                drop(msg);
                SendOutcome::Dropped
            }
        }
    }

    pub fn try_recv_mic(&self) -> Option<BusMsg> {
        lock(&self.inner.mic).take()
    }

    pub fn try_recv_utterance(&self) -> Option<BusMsg> {
        lock(&self.inner.utterances).pop_front()
    }

    pub fn try_recv_reply(&self) -> Option<BusMsg> {
        lock(&self.inner.replies).pop_front()
    }

    pub fn try_recv_consent(&self) -> Option<BusMsg> {
        lock(&self.inner.consent).take()
    }

    /// Drop a stale utterance before any ASR work. `BargeIn` only moves cancel.
    pub fn before_asr(&self, mut msg: BusMsg) -> Option<BusMsg> {
        self.observe_cancel(&msg);
        if matches!(msg.body, BusBody::BargeIn { .. }) {
            return None;
        }
        if !admit(&self.inner, msg.turn_id, false) {
            return None;
        }
        msg.cancel = self.cancel();
        Some(msg)
    }

    /// Drop a stale partial or reply before it is published.
    pub fn before_publish(&self, mut msg: BusMsg) -> Option<BusMsg> {
        self.observe_cancel(&msg);
        if !admit(&self.inner, msg.turn_id, true) {
            return None;
        }
        msg.cancel = self.cancel();
        Some(msg)
    }

    /// ASR gate, then the publish gate. `asr` is not called for a stale turn.
    /// `publish` is not called when barge-in wins during ASR.
    pub fn run_two_stages(
        &self,
        utterance: BusMsg,
        mut asr: impl FnMut(&BusMsg) -> BusMsg,
        mut publish: impl FnMut(&BusMsg) -> BusMsg,
    ) -> StageResult {
        let Some(u) = self.before_asr(utterance) else {
            return StageResult::DroppedBeforeAsr;
        };
        let partial = asr(&u);
        let Some(partial) = self.before_publish(partial) else {
            return StageResult::DroppedBeforePublish;
        };
        let reply = publish(&partial);
        let Some(reply) = self.before_publish(reply) else {
            return StageResult::DroppedBeforePublish;
        };
        let _ = self.try_send_reply(reply);
        StageResult::Published
    }

    fn observe_cancel(&self, msg: &BusMsg) {
        self.raise_cancel(msg.cancel);
        if let BusBody::BargeIn { turn_id } = msg.body {
            self.raise_cancel(turn_id);
        }
    }

    fn raise_cancel(&self, turn_id: u64) {
        if turn_id == 0 {
            return;
        }
        let prev = self.inner.cancel.fetch_max(turn_id, Ordering::AcqRel);
        if turn_id > prev {
            self.drop_stale();
        }
    }

    fn drop_stale(&self) {
        let c = self.cancel();
        let _ = clear_slot(&self.inner.mic, c);
        let _ = clear_slot(&self.inner.orb, c);
        let _ = clear_slot(&self.inner.consent, c);
        let n = retain_fifo(&self.inner.utterances, c);
        self.inner
            .dropped_before_asr
            .fetch_add(n, Ordering::Relaxed);
        let n = retain_fifo(&self.inner.partials, c) + retain_fifo(&self.inner.replies, c);
        self.inner
            .dropped_before_publish
            .fetch_add(n, Ordering::Relaxed);
        let _ = retain_fifo(&self.inner.faults, c);
    }

    fn enqueue_slot(&self, slot: &Mutex<Option<BusMsg>>, msg: BusMsg) -> SendOutcome {
        let mut g = lock(slot);
        if msg.is_stale(self.cancel()) {
            return SendOutcome::Dropped;
        }
        let replaced = g.is_some();
        *g = Some(msg);
        if replaced {
            SendOutcome::Replaced
        } else {
            SendOutcome::Queued
        }
    }

    fn enqueue_fifo(
        &self,
        q: &Mutex<VecDeque<BusMsg>>,
        depth: usize,
        msg: BusMsg,
        stale: Stale,
    ) -> SendOutcome {
        let mut q = lock(q);
        if msg.is_stale(self.cancel()) {
            drop(q);
            match stale {
                Stale::Asr => {
                    self.inner
                        .dropped_before_asr
                        .fetch_add(1, Ordering::Relaxed);
                }
                Stale::Publish => {
                    self.inner
                        .dropped_before_publish
                        .fetch_add(1, Ordering::Relaxed);
                }
                Stale::Quiet => {}
            }
            return SendOutcome::Dropped;
        }
        let mut evicted = false;
        while q.len() >= depth {
            q.pop_front();
            evicted = true;
        }
        q.push_back(msg);
        if evicted {
            SendOutcome::EvictedOldest
        } else {
            SendOutcome::Queued
        }
    }
}

impl Default for ProcessBus {
    fn default() -> Self {
        Self::new()
    }
}

enum Stale {
    Asr,
    Publish,
    Quiet,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn clear_slot(slot: &Mutex<Option<BusMsg>>, cancel: u64) -> bool {
    let mut g = lock(slot);
    if g.as_ref().is_some_and(|m| m.is_stale(cancel)) {
        *g = None;
        true
    } else {
        false
    }
}

fn retain_fifo(q: &Mutex<VecDeque<BusMsg>>, cancel: u64) -> u64 {
    let mut q = lock(q);
    let before = q.len();
    q.retain(|m| !m.is_stale(cancel));
    (before - q.len()) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    fn utt(bus: &ProcessBus, turn: u64) -> BusMsg {
        BusMsg::utterance(
            bus.alloc_id(),
            turn,
            turn.saturating_mul(80),
            bus.cancel(),
            16_000,
            vec![0, 1],
        )
    }

    #[test]
    fn supersede_drops_before_asr_and_before_reply_publish() {
        let bus = ProcessBus::new();
        let asr_ran = AtomicBool::new(false);
        let publish_ran = AtomicBool::new(false);
        let during = bus.clone();
        let outcome = bus.run_two_stages(
            utt(&bus, 1),
            |u| {
                asr_ran.store(true, Ordering::Relaxed);
                during.supersede(2);
                BusMsg::partial_asr(
                    during.alloc_id(),
                    u.turn_id,
                    u.t_ms,
                    during.cancel(),
                    "hello",
                    ASR_INTERIM_PLACEHOLDER,
                    false,
                )
            },
            |_| {
                publish_ran.store(true, Ordering::Relaxed);
                BusMsg::reply_text(0, 1, 0, 0, "nope", None, false)
            },
        );
        assert!(
            asr_ran.load(Ordering::Relaxed),
            "ASR runs for the current turn"
        );
        assert!(
            !publish_ran.load(Ordering::Relaxed),
            "publish does not run after barge-in"
        );
        assert_eq!(outcome, StageResult::DroppedBeforePublish);
        assert!(bus.dropped_before_publish() >= 1);
        assert_eq!(bus.reply_len(), 0);
        assert_eq!(bus.cancel(), 2);

        let late_ran = AtomicBool::new(false);
        let outcome = bus.run_two_stages(
            utt(&bus, 1),
            |_| {
                late_ran.store(true, Ordering::Relaxed);
                BusMsg::partial_asr(0, 1, 0, 0, "", ASR_INTERIM_PLACEHOLDER, true)
            },
            |_| BusMsg::reply_text(0, 1, 0, 0, "", None, false),
        );
        assert!(
            !late_ran.load(Ordering::Relaxed),
            "stale utterance never reaches ASR"
        );
        assert_eq!(outcome, StageResult::DroppedBeforeAsr);
        assert!(bus.dropped_before_asr() >= 1);

        let published = bus.clone();
        let outcome = bus.run_two_stages(
            utt(&bus, 2),
            |u| {
                BusMsg::partial_asr(
                    published.alloc_id(),
                    u.turn_id,
                    u.t_ms,
                    published.cancel(),
                    "hello",
                    ASR_INTERIM_PLACEHOLDER,
                    true,
                )
            },
            |p| {
                match &p.body {
                    BusBody::PartialAsr {
                        source, is_final, ..
                    } => {
                        assert_eq!(*source, ASR_INTERIM_PLACEHOLDER);
                        assert!(*is_final);
                        assert!(asr_source_is_placeholder(source));
                    }
                    other => panic!("expected partial, got {other:?}"),
                }
                BusMsg::reply_text(
                    published.alloc_id(),
                    p.turn_id,
                    p.t_ms,
                    published.cancel(),
                    "Hi",
                    Some("warm".into()),
                    false,
                )
            },
        );
        assert_eq!(outcome, StageResult::Published);
        let reply = bus.try_recv_reply().expect("current turn published");
        assert_eq!(reply.turn_id, 2);
        match reply.body {
            BusBody::ReplyText {
                text,
                emotion: None,
            } => assert_eq!(text, "Hi"),
            other => panic!("emotion must be stripped without sense, got {other:?}"),
        }
    }

    #[test]
    fn barge_in_sets_cancel_and_drops_queued_utterance() {
        let bus = ProcessBus::new();
        assert_eq!(bus.try_send(utt(&bus, 1)).unwrap(), SendOutcome::Queued);
        assert_eq!(bus.try_send(utt(&bus, 2)).unwrap(), SendOutcome::Queued);
        assert_eq!(bus.utterance_len(), 2);
        assert_eq!(
            bus.try_send(BusMsg::barge_in(bus.alloc_id(), 2, 160))
                .unwrap(),
            SendOutcome::Applied
        );
        assert_eq!(bus.cancel(), 2);
        assert_eq!(bus.utterance_len(), 1);
        assert_eq!(bus.try_recv_utterance().unwrap().turn_id, 2);
        assert!(bus.dropped_before_asr() >= 1);
    }

    #[test]
    fn backpressure_matches_ipc_caps() {
        use galion_render::backends::web::{MAX_BODY_BYTES, MAX_PENDING_TURNS, MAX_STORED_REPLIES};
        assert_eq!(UTTERANCE_DEPTH, MAX_PENDING_TURNS);
        assert_eq!(REPLY_DEPTH, MAX_STORED_REPLIES);
        assert_eq!(MAX_UTTERANCE_BYTES, MAX_BODY_BYTES);
        assert_eq!((MIC_DEPTH, JOB_DEPTH, WORKER_REPLY_DEPTH), (1, 1, 4));

        let bus = ProcessBus::new();
        assert_eq!(
            bus.try_send(BusMsg::mic_level(1, 0, 0.1)).unwrap(),
            SendOutcome::Queued
        );
        assert_eq!(
            bus.try_send(BusMsg::mic_level(2, 80, 0.4)).unwrap(),
            SendOutcome::Replaced
        );
        assert_eq!(
            bus.try_send(BusMsg::mic_level(3, 160, 0.2)).unwrap(),
            SendOutcome::Replaced
        );
        assert_eq!(bus.mic_len(), MIC_DEPTH);
        match bus.try_recv_mic().unwrap().body {
            BusBody::MicLevel { rms } => assert!((rms - 0.2).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
        let nan = BusMsg::mic_level(4, 0, f32::NAN);
        let hot = BusMsg::mic_level(5, 0, 2.0);
        match (nan.body, hot.body) {
            (BusBody::MicLevel { rms: a }, BusBody::MicLevel { rms: b }) => {
                assert_eq!((a, b), (0.0, 1.0));
            }
            _ => panic!("mic constructors"),
        }

        let bus = ProcessBus::new();
        for turn in 1..=10 {
            let outcome = bus
                .try_send(BusMsg::utterance(
                    bus.alloc_id(),
                    turn,
                    0,
                    0,
                    16_000,
                    vec![0, 0],
                ))
                .unwrap();
            if turn > UTTERANCE_DEPTH as u64 {
                assert_eq!(outcome, SendOutcome::EvictedOldest);
            }
        }
        assert_eq!(bus.utterance_len(), UTTERANCE_DEPTH);
        assert_eq!(bus.try_recv_utterance().unwrap().turn_id, 3);

        let big = vec![0u8; MAX_UTTERANCE_BYTES + 1];
        let err = bus
            .try_send(BusMsg::utterance(1, 1, 0, 0, 16_000, big))
            .unwrap_err();
        assert!(matches!(err, SendError::TooLarge(_)));

        let bus = ProcessBus::new();
        for turn in 1..=10 {
            let msg = BusMsg::speech_pcm(bus.alloc_id(), turn, 0, 0, vec![0, 0], 1);
            let outcome = bus.try_send_reply(msg);
            if turn > REPLY_DEPTH as u64 {
                assert_eq!(outcome, SendOutcome::EvictedOldest);
            }
        }
        assert_eq!(bus.reply_len(), REPLY_DEPTH);
        assert_eq!(bus.try_recv_reply().unwrap().turn_id, 3);
        match BusMsg::speech_pcm(1, 1, 0, 0, vec![], 1).body {
            BusBody::SpeechPcm { rate, .. } => assert_eq!(rate, SPEECH_RATE_HZ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn full_job_and_worker_reply_lanes_do_not_block() {
        let bus = ProcessBus::new();
        bus.try_send_job(utt(&bus, 1)).unwrap();
        let t = Instant::now();
        let rejected = bus
            .try_send_job(utt(&bus, 2))
            .expect_err("depth 1 returns the job");
        assert!(
            t.elapsed() < Duration::from_millis(20),
            "job try_send waited {:?}",
            t.elapsed()
        );
        assert_eq!(rejected.turn_id, 2);

        for turn in 1..=WORKER_REPLY_DEPTH as u64 {
            let msg = BusMsg::speech_pcm(bus.alloc_id(), turn, 0, 0, vec![0, 0], 1);
            assert_eq!(bus.try_send_worker_reply(msg), SendOutcome::Queued);
        }
        let t = Instant::now();
        let msg = BusMsg::speech_pcm(bus.alloc_id(), 99, 0, 0, vec![0, 0], 1);
        assert_eq!(bus.try_send_worker_reply(msg), SendOutcome::Dropped);
        assert!(
            t.elapsed() < Duration::from_millis(20),
            "worker reply try_send waited {:?}",
            t.elapsed()
        );
        assert_eq!(bus.worker_reply_queued(), WORKER_REPLY_DEPTH);
        assert_eq!(bus.worker_replies_dropped(), 1);
    }

    #[test]
    fn full_job_lane_does_not_block_heartbeat_step() {
        use galion_emotion::{DefaultConsentGate, Purpose};
        use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_SAMPLES};

        let bus = ProcessBus::new();
        bus.try_send_job(utt(&bus, 1)).unwrap();
        assert!(bus.try_send_job(utt(&bus, 2)).is_err());

        let mut hb = Heartbeat::new(HeartbeatConfig::default(), DefaultConsentGate::default());
        let you = vec![0f32; FRAME_SAMPLES];
        let frame = FrameIn {
            you_pcm: &you,
            affect: None,
            tension: None,
            consent_bits: 0,
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: false,
            late: false,
        };
        let t = Instant::now();
        let _out = hb.step(&frame);
        assert!(
            t.elapsed() < Duration::from_millis(20),
            "Heartbeat::step waited {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn consent_defaults_deny_and_emotion_needs_sense() {
        let bus = ProcessBus::new();
        bus.try_send(BusMsg::consent_deny(bus.alloc_id(), 0))
            .unwrap();
        match bus.try_recv_consent().unwrap().body {
            BusBody::Consent { bits, purpose } => {
                assert_eq!(bits, 0);
                assert_eq!(purpose, PURPOSE_NONE);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(CONSENT_SENSE, galion_emotion::band::consent_bits::SENSE);
        let granted = BusMsg::reply_text(1, 1, 0, 0, "hi", Some("warm".into()), true);
        let denied = BusMsg::reply_text(2, 1, 0, 0, "hi", Some("warm".into()), false);
        match (granted.body, denied.body) {
            (
                BusBody::ReplyText {
                    emotion: Some(tag), ..
                },
                BusBody::ReplyText { emotion: None, .. },
            ) => {
                assert_eq!(tag, "warm");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn interim_label_is_not_recognition_and_orb_modes_match() {
        assert_eq!(ASR_INTERIM_PLACEHOLDER, "interim-placeholder");
        assert_eq!(ASR_STUB_FIXTURE, "stub-fixture");
        assert!(asr_source_is_placeholder(ASR_INTERIM_PLACEHOLDER));
        assert_ne!(ASR_INTERIM_PLACEHOLDER, "recognition");
        assert!(!asr_source_is_placeholder("recognition"));

        for motion in [
            galion_render::OrbMotion::Idle,
            galion_render::OrbMotion::Listening,
            galion_render::OrbMotion::Thinking,
            galion_render::OrbMotion::Speaking,
            galion_render::OrbMotion::Offline,
        ] {
            assert_eq!(OrbMode::from(motion).as_str(), motion.as_str());
        }
        let msg = BusMsg::fault(1, 4, 80, 4, "asr", "AsrUnratified");
        match msg.body {
            BusBody::Fault { stage, kind } => {
                assert_eq!(stage, "asr");
                assert_eq!(kind, "AsrUnratified");
            }
            other => panic!("{other:?}"),
        }
    }
}
