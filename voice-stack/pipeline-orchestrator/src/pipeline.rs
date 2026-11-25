// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Tokio conversation loop.
//!
//! ```text
//! mic -> frames (bound 8) -> listen / VAD
//!     -> turns (bound 1, newest wins) -> think (ASR, reply, clauses)
//!     -> chunks (bound 4) -> speak
//! ```
//!
//! Barge-in does not wait on the chunk channel. On voice start during
//! thinking or speaking, the listen task bumps the epoch, sets the cancel
//! flag, and calls [`AudioOut::stop`] before it returns to the mic.
//!
//! Phases match `galion_console::bus::OrbMode`: idle, listening, thinking,
//! speaking. Subscribe with [`Handle::subscribe`].

use crate::clause::clauses;
use crate::clock::{
    CHUNK_CHANNEL_BOUND, FRAME_CHANNEL_BOUND, HEARTBEAT_MS, HEARTBEAT_PLAY_SAMPLES,
    MAX_UTTERANCE_MS, TURN_CHANNEL_BOUND,
};
// Demo uses these without naming the stage-trait module.
pub(crate) use crate::voice_traits::{mic_channel, WaitGate};
use crate::voice_traits::{
    Asr, AudioFrame, AudioIn, AudioOut, MoodTick, Pad, ReplyEngine, Tts, Vad, VadEvent,
};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{mpsc, watch, Notify};
use tokio::task::JoinHandle;

/// Orb phases. Same names as the one-dot console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Listening => "listening",
            Phase::Thinking => "thinking",
            Phase::Speaking => "speaking",
        }
    }
}

/// One finished or cancelled turn. Times are wall clock except
/// `listen_audio_ms`, which is the sum of capture-frame durations.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnReport {
    pub turn_id: u64,
    pub listen_audio_ms: u32,
    pub asr_us: u32,
    pub think_us: u32,
    pub first_audio_us: Option<u32>,
    pub eot_to_audio_us: Option<u32>,
    pub chunks_started: u32,
    pub barge_us: Option<u32>,
    pub spoken: bool,
    pub stale: bool,
}

/// Mood published on the 80 ms tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodSnap {
    pub pad: Pad,
    pub frame: u64,
    pub advance_us: u32,
    pub at_ms: u64,
}

/// In-process subscription. The HTML dot polls the same fields over HTTP.
#[derive(Clone, Debug)]
pub struct Subscribe {
    phase: watch::Receiver<Phase>,
    mood: watch::Receiver<MoodSnap>,
}

impl Subscribe {
    pub fn phase(&self) -> Phase {
        *self.phase.borrow()
    }

    pub fn mood(&self) -> MoodSnap {
        *self.mood.borrow()
    }

    pub fn phase_receiver(&self) -> watch::Receiver<Phase> {
        self.phase.clone()
    }

    #[cfg(test)]
    pub(crate) fn from_receivers(
        phase: watch::Receiver<Phase>,
        mood: watch::Receiver<MoodSnap>,
    ) -> Self {
        Self { phase, mood }
    }

    pub fn mood_receiver(&self) -> watch::Receiver<MoodSnap> {
        self.mood.clone()
    }
}

/// Running loop.
pub struct Handle {
    phase_rx: watch::Receiver<Phase>,
    mood_rx: watch::Receiver<MoodSnap>,
    shutdown_tx: watch::Sender<bool>,
    /// Same flag the mouth watches so a blocked `speak` can leave on shutdown.
    stop: Arc<AtomicBool>,
    joins: Mutex<Vec<JoinHandle<()>>>,
    reports: Arc<Mutex<Vec<TurnReport>>>,
    phases: Arc<Mutex<Vec<Phase>>>,
}

impl Handle {
    pub fn phase(&self) -> Phase {
        *self.phase_rx.borrow()
    }

    pub fn mood(&self) -> MoodSnap {
        *self.mood_rx.borrow()
    }

    pub fn subscribe(&self) -> Subscribe {
        Subscribe {
            phase: self.phase_rx.clone(),
            mood: self.mood_rx.clone(),
        }
    }

    pub fn reports(&self) -> Vec<TurnReport> {
        self.reports.lock().expect("reports").clone()
    }

    pub fn phases(&self) -> Vec<Phase> {
        self.phases.lock().expect("phases").clone()
    }

    pub async fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(true);
        let joins: Vec<_> = self.joins.get_mut().expect("joins").drain(..).collect();
        for join in joins {
            let _ = join.await;
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // A test that panics while the mouth is parked on its gate would
        // otherwise pin a worker inside `thread::sleep`. Abort does not
        // interrupt that loop; the cancel flag does.
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(true);
        if let Ok(joins) = self.joins.get_mut() {
            for join in joins.drain(..) {
                join.abort();
            }
        }
    }
}

/// The six stages plus the mood tick.
pub struct Pipeline<I, V, A, R, T, O, M> {
    pub mic: I,
    pub vad: V,
    pub asr: A,
    pub reply: R,
    pub tts: T,
    pub speaker: O,
    pub mood: M,
}

impl<I, V, A, R, T, O, M> Pipeline<I, V, A, R, T, O, M>
where
    I: AudioIn,
    V: Vad,
    A: Asr,
    R: ReplyEngine,
    T: Tts,
    O: AudioOut,
    M: MoodTick,
{
    pub fn spawn(self) -> Handle {
        let Pipeline {
            mic,
            vad,
            asr,
            reply,
            tts,
            speaker,
            mood,
        } = self;

        let (phase_tx, phase_rx) = watch::channel(Phase::Idle);
        let (mood_tx, mood_rx) = watch::channel(MoodSnap {
            pad: Pad::RESTING,
            frame: 0,
            advance_us: 0,
            at_ms: 0,
        });
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let reports = Arc::new(Mutex::new(Vec::new()));
        let phases = Arc::new(Mutex::new(Vec::new()));
        let floor = Arc::new(Floor {
            state: Mutex::new(FloorState {
                phase: Phase::Idle,
                epoch: 1,
            }),
            epoch: Arc::new(AtomicU64::new(1)),
            stop: Arc::new(AtomicBool::new(false)),
            last_barge_us: Arc::new(AtomicU32::new(0)),
            phase_tx,
            phases: phases.clone(),
            notify: Arc::new(Notify::new()),
        });
        let speaker = SharedSpeaker {
            inner: Arc::new(Mutex::new(speaker)),
            stop: floor.stop.clone(),
            epoch: floor.epoch.clone(),
        };
        let mood = Arc::new(Mutex::new(mood));
        let (frame_tx, frame_rx) = mpsc::channel(FRAME_CHANNEL_BOUND);
        let (turns, turn_rx) = TurnMailbox::pair();
        let (chunk_tx, chunk_rx) = mpsc::channel(CHUNK_CHANNEL_BOUND);

        let joins = vec![
            tokio::spawn(capture(mic, frame_tx, shutdown_rx.clone())),
            tokio::spawn(listen(
                vad,
                frame_rx,
                turns,
                floor.clone(),
                speaker.clone(),
                shutdown_rx.clone(),
            )),
            tokio::spawn(think(ThinkIn {
                asr,
                reply,
                turns: turn_rx,
                chunks: chunk_tx,
                floor: floor.clone(),
                mood: mood.clone(),
                reports: reports.clone(),
                shutdown: shutdown_rx.clone(),
            })),
            tokio::spawn(speak(
                tts,
                chunk_rx,
                speaker,
                floor.clone(),
                reports.clone(),
                shutdown_rx.clone(),
            )),
            tokio::spawn(mood_tick(mood, mood_tx, shutdown_rx)),
        ];

        Handle {
            phase_rx,
            mood_rx,
            shutdown_tx,
            stop: floor.stop.clone(),
            joins: Mutex::new(joins),
            reports,
            phases,
        }
    }
}

struct FloorState {
    phase: Phase,
    epoch: u64,
}

struct Floor {
    state: Mutex<FloorState>,
    epoch: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    last_barge_us: Arc<AtomicU32>,
    phase_tx: watch::Sender<Phase>,
    phases: Arc<Mutex<Vec<Phase>>>,
    notify: Arc<Notify>,
}

impl Floor {
    fn set_any(&self, next: Phase) {
        let mut state = self.state.lock().expect("floor");
        if state.phase == next {
            return;
        }
        state.phase = next;
        let _ = self.phase_tx.send(next);
        self.phases.lock().expect("phases").push(next);
    }

    fn set_epoch(&self, epoch: u64, next: Phase) -> bool {
        let mut state = self.state.lock().expect("floor");
        if state.epoch != epoch {
            return false;
        }
        if state.phase == next {
            return true;
        }
        state.phase = next;
        let _ = self.phase_tx.send(next);
        self.phases.lock().expect("phases").push(next);
        true
    }

    /// Voice during thinking or speaking. Cancel runs before this returns.
    fn barge<O: AudioOut>(&self, speaker: &SharedSpeaker<O>) -> u32 {
        let started = Instant::now();
        {
            let mut state = self.state.lock().expect("floor");
            state.epoch = state.epoch.wrapping_add(1);
            self.epoch.store(state.epoch, Ordering::SeqCst);
            self.stop.store(true, Ordering::SeqCst);
            if state.phase != Phase::Listening {
                state.phase = Phase::Listening;
                let _ = self.phase_tx.send(Phase::Listening);
                self.phases.lock().expect("phases").push(Phase::Listening);
            }
        }
        speaker.stop();
        let elapsed = us(started.elapsed());
        self.last_barge_us.store(elapsed, Ordering::Relaxed);
        self.notify.notify_waiters();
        elapsed
    }
}

struct SharedSpeaker<O> {
    inner: Arc<Mutex<O>>,
    stop: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
}

impl<O> Clone for SharedSpeaker<O> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            stop: Arc::clone(&self.stop),
            epoch: Arc::clone(&self.epoch),
        }
    }
}

impl<O: AudioOut> SharedSpeaker<O> {
    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.inner.lock().expect("speaker").stop();
    }

    fn begin_turn(&self, epoch: u64) {
        if self.epoch.load(Ordering::SeqCst) == epoch {
            self.stop.store(false, Ordering::SeqCst);
        }
    }

    fn write(&self, epoch: u64, pcm: &[i16]) -> bool {
        if self.stop.load(Ordering::SeqCst) || self.epoch.load(Ordering::SeqCst) != epoch {
            return false;
        }
        if pcm.is_empty() {
            return true;
        }
        self.inner.lock().expect("speaker").write(pcm);
        true
    }
}

struct Heard {
    turn_id: u64,
    epoch: u64,
    frames: Vec<AudioFrame>,
    listen_audio_ms: u32,
    ended_at: Instant,
}

#[derive(Clone)]
struct TurnMeta {
    turn_id: u64,
    epoch: u64,
    ready_at: Instant,
    ended_at: Instant,
    listen_audio_ms: u32,
    asr_us: u32,
    think_us: u32,
}

enum SpeakItem {
    Chunk {
        meta: TurnMeta,
        index: u32,
        text: String,
        mood: Pad,
    },
    End {
        meta: TurnMeta,
    },
}

/// Bound-1 turn queue. A newer end-of-turn replaces the one slot that did not fit.
struct TurnMailbox {
    tx: mpsc::Sender<Heard>,
    overflow: Arc<Mutex<Option<Heard>>>,
    notify: Arc<Notify>,
}

struct TurnInbox {
    rx: mpsc::Receiver<Heard>,
    overflow: Arc<Mutex<Option<Heard>>>,
    notify: Arc<Notify>,
}

impl TurnMailbox {
    fn pair() -> (Self, TurnInbox) {
        let (tx, rx) = mpsc::channel(TURN_CHANNEL_BOUND);
        let overflow = Arc::new(Mutex::new(None));
        let notify = Arc::new(Notify::new());
        (
            Self {
                tx,
                overflow: overflow.clone(),
                notify: notify.clone(),
            },
            TurnInbox {
                rx,
                overflow,
                notify,
            },
        )
    }

    fn publish(&self, heard: Heard) -> bool {
        match self.tx.try_send(heard) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(heard)) => {
                *self.overflow.lock().expect("overflow") = Some(heard);
                self.notify.notify_one();
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }
}

impl TurnInbox {
    fn take_ready(&mut self) -> Option<Heard> {
        let queued = self.rx.try_recv().ok();
        let parked = self.overflow.lock().expect("overflow").take();
        match (queued, parked) {
            (None, None) => None,
            (Some(queued), None) => Some(queued),
            (None, Some(parked)) | (Some(_), Some(parked)) => Some(parked),
        }
    }

    async fn recv(&mut self) -> Option<Heard> {
        let notify = self.notify.clone();
        loop {
            if let Some(heard) = self.take_ready() {
                let mut current = heard;
                while let Some(newer) = self.take_ready() {
                    current = newer;
                }
                return Some(current);
            }
            tokio::select! {
                _ = notify.notified() => continue,
                heard = self.rx.recv() => {
                    let heard = heard?;
                    let mut current = self
                        .overflow
                        .lock()
                        .expect("overflow")
                        .take()
                        .unwrap_or(heard);
                    while let Some(newer) = self.take_ready() {
                        current = newer;
                    }
                    return Some(current);
                }
            }
        }
    }
}

async fn capture<I: AudioIn>(
    mut mic: I,
    frame_tx: mpsc::Sender<AudioFrame>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        let frame = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            frame = mic.recv() => frame,
        };
        let Some(frame) = frame else {
            break;
        };
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            sent = frame_tx.send(frame) => {
                if sent.is_err() {
                    break;
                }
            }
        }
    }
}

async fn listen<V: Vad, O: AudioOut>(
    mut vad: V,
    mut frames: mpsc::Receiver<AudioFrame>,
    turns: TurnMailbox,
    floor: Arc<Floor>,
    speaker: SharedSpeaker<O>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut hearing = false;
    let mut buf: Vec<AudioFrame> = Vec::new();
    let mut preroll: std::collections::VecDeque<AudioFrame> = std::collections::VecDeque::new();
    let mut next_turn = 1u64;

    loop {
        let frame = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            frame = frames.recv() => frame,
        };
        let Some(frame) = frame else {
            break;
        };
        preroll.push_back(frame.clone());
        while preroll.len() > 8 {
            preroll.pop_front();
        }
        let event = vad.push(&frame);
        match event {
            VadEvent::Start => {
                let phase = *floor.phase_tx.borrow();
                if phase == Phase::Thinking || phase == Phase::Speaking {
                    floor.barge(&speaker);
                }
                hearing = true;
                buf = preroll.iter().cloned().collect();
                floor.set_any(Phase::Listening);
            }
            VadEvent::None if hearing => {
                buf.push(frame);
                let audio_ms = buf.iter().map(|f| f.ms).sum::<u32>();
                if audio_ms >= MAX_UTTERANCE_MS {
                    vad.reset_turn();
                    hearing = false;
                    emit_turn(&turns, &floor, &mut buf, &mut next_turn);
                }
            }
            VadEvent::End if hearing => {
                hearing = false;
                buf.push(frame);
                emit_turn(&turns, &floor, &mut buf, &mut next_turn);
            }
            _ => {}
        }
    }
}

fn emit_turn(turns: &TurnMailbox, floor: &Floor, buf: &mut Vec<AudioFrame>, next_turn: &mut u64) {
    let frames = std::mem::take(buf);
    if frames.is_empty() {
        return;
    }
    let listen_audio_ms = frames.iter().map(|f| f.ms).sum();
    let epoch = floor.epoch.load(Ordering::SeqCst);
    let turn_id = *next_turn;
    *next_turn = next_turn.saturating_add(1);
    let heard = Heard {
        turn_id,
        epoch,
        frames,
        listen_audio_ms,
        ended_at: Instant::now(),
    };
    // Thinking has to be on the watch before the turn is visible. `publish`
    // wakes think on another worker, and that worker can reach Speaking
    // before this call returns. A later Thinking send replaces Speaking
    // on the watch while the mouth is still inside the gated speak, so
    // `/state` and `Handle::phase` both stay on Thinking.
    let _ = floor.set_epoch(epoch, Phase::Thinking);
    let _ = turns.publish(heard);
}

struct ThinkIn<A, R, M> {
    asr: A,
    reply: R,
    turns: TurnInbox,
    chunks: mpsc::Sender<SpeakItem>,
    floor: Arc<Floor>,
    mood: Arc<Mutex<M>>,
    reports: Arc<Mutex<Vec<TurnReport>>>,
    shutdown: watch::Receiver<bool>,
}

async fn think<A: Asr, R: ReplyEngine, M: MoodTick>(inn: ThinkIn<A, R, M>) {
    let ThinkIn {
        mut asr,
        mut reply,
        mut turns,
        chunks,
        floor,
        mood,
        reports,
        mut shutdown,
    } = inn;
    loop {
        let heard = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            heard = turns.recv() => heard,
        };
        let Some(heard) = heard else {
            break;
        };
        if floor.epoch.load(Ordering::SeqCst) != heard.epoch {
            push_stale(&reports, &floor, &heard, 0, 0);
            continue;
        }
        let asr_started = Instant::now();
        asr.reset();
        for frame in &heard.frames {
            asr.push(frame);
        }
        let transcript = asr.finish();
        let asr_us = us(asr_started.elapsed());
        if floor.epoch.load(Ordering::SeqCst) != heard.epoch {
            push_stale(&reports, &floor, &heard, asr_us, 0);
            continue;
        }
        let text = transcript.text.trim();
        if text.is_empty() {
            floor.set_epoch(heard.epoch, Phase::Idle);
            continue;
        }
        let tick = mood.lock().expect("mood").get();
        let think_started = Instant::now();
        let answered = reply.turn(text, tick);
        let think_us = us(think_started.elapsed());
        if floor.epoch.load(Ordering::SeqCst) != heard.epoch {
            push_stale(&reports, &floor, &heard, asr_us, think_us);
            continue;
        }
        mood.lock().expect("mood").set(answered.mood);
        let parts = clauses(&answered.text);
        if parts.is_empty() {
            floor.set_epoch(heard.epoch, Phase::Idle);
            continue;
        }
        let meta = TurnMeta {
            turn_id: heard.turn_id,
            epoch: heard.epoch,
            ready_at: Instant::now(),
            ended_at: heard.ended_at,
            listen_audio_ms: heard.listen_audio_ms,
            asr_us,
            think_us,
        };
        for (index, text) in parts.into_iter().enumerate() {
            if floor.epoch.load(Ordering::SeqCst) != meta.epoch {
                break;
            }
            let item = SpeakItem::Chunk {
                meta: meta.clone(),
                index: index as u32,
                text,
                mood: answered.mood,
            };
            let send = chunks.send(item);
            tokio::pin!(send);
            tokio::select! {
                _ = floor.notify.notified() => break,
                result = &mut send => {
                    if result.is_err() {
                        return;
                    }
                }
            }
        }
        let end = SpeakItem::End { meta };
        if chunks.send(end).await.is_err() {
            break;
        }
    }
}

fn push_stale(
    reports: &Mutex<Vec<TurnReport>>,
    floor: &Floor,
    heard: &Heard,
    asr_us: u32,
    think_us: u32,
) {
    let barge_us = floor.last_barge_us.load(Ordering::Relaxed);
    reports.lock().expect("reports").push(TurnReport {
        turn_id: heard.turn_id,
        listen_audio_ms: heard.listen_audio_ms,
        asr_us,
        think_us,
        first_audio_us: None,
        eot_to_audio_us: None,
        chunks_started: 0,
        barge_us: Some(barge_us),
        spoken: false,
        stale: true,
    });
}

async fn speak<T: Tts, O: AudioOut>(
    mut tts: T,
    mut chunks: mpsc::Receiver<SpeakItem>,
    speaker: SharedSpeaker<O>,
    floor: Arc<Floor>,
    reports: Arc<Mutex<Vec<TurnReport>>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut progress: Option<SpeakProg> = None;
    loop {
        let item = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            item = chunks.recv() => item,
        };
        let Some(item) = item else {
            break;
        };
        match item {
            SpeakItem::Chunk {
                meta,
                index,
                text,
                mood,
            } => {
                if floor.epoch.load(Ordering::SeqCst) != meta.epoch {
                    continue;
                }
                if index == 0 {
                    speaker.begin_turn(meta.epoch);
                    progress = Some(SpeakProg {
                        turn_id: meta.turn_id,
                        first_audio_us: None,
                        eot_to_audio_us: None,
                        chunks_started: 0,
                    });
                    floor.set_epoch(meta.epoch, Phase::Speaking);
                }
                let Some(prog) = progress.as_mut() else {
                    continue;
                };
                if prog.turn_id != meta.turn_id {
                    continue;
                }
                prog.chunks_started = prog.chunks_started.saturating_add(1);
                let stop = floor.stop.clone();
                let epoch = meta.epoch;
                let ready_at = meta.ready_at;
                let ended_at = meta.ended_at;
                tts.speak(&text, &mood, &stop, &mut |pcm| {
                    // 80 ms slices. The first write is the first slice, before the
                    // rest of this clause. Cancel is the same flag the mouth polls.
                    let mut off = 0;
                    while off < pcm.len() {
                        if stop.load(Ordering::SeqCst) {
                            return;
                        }
                        let end = (off + HEARTBEAT_PLAY_SAMPLES).min(pcm.len());
                        if !speaker.write(epoch, &pcm[off..end]) {
                            return;
                        }
                        if prog.first_audio_us.is_none() {
                            prog.first_audio_us = Some(us(ready_at.elapsed()));
                            prog.eot_to_audio_us = Some(us(ended_at.elapsed()));
                        }
                        off = end;
                    }
                });
            }
            SpeakItem::End { meta } => {
                let prog = progress.take().filter(|p| p.turn_id == meta.turn_id);
                let stale = floor.epoch.load(Ordering::SeqCst) != meta.epoch;
                let first_audio_us = prog.as_ref().and_then(|p| p.first_audio_us);
                let eot_to_audio_us = prog.as_ref().and_then(|p| p.eot_to_audio_us);
                let chunks_started = prog.map(|p| p.chunks_started).unwrap_or(0);
                let barge_us = if stale {
                    Some(floor.last_barge_us.load(Ordering::Relaxed))
                } else {
                    None
                };
                reports.lock().expect("reports").push(TurnReport {
                    turn_id: meta.turn_id,
                    listen_audio_ms: meta.listen_audio_ms,
                    asr_us: meta.asr_us,
                    think_us: meta.think_us,
                    first_audio_us,
                    eot_to_audio_us,
                    chunks_started,
                    barge_us,
                    spoken: !stale && first_audio_us.is_some(),
                    stale,
                });
                if !stale {
                    floor.set_epoch(meta.epoch, Phase::Idle);
                }
            }
        }
    }
}

struct SpeakProg {
    turn_id: u64,
    first_audio_us: Option<u32>,
    eot_to_audio_us: Option<u32>,
    chunks_started: u32,
}

async fn mood_tick<M: MoodTick>(
    mood: Arc<Mutex<M>>,
    tx: watch::Sender<MoodSnap>,
    mut shutdown: watch::Receiver<bool>,
) {
    let started = Instant::now();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(HEARTBEAT_MS as u64));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut frame = 0u64;
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                let t0 = Instant::now();
                let pad = mood.lock().expect("mood").advance(HEARTBEAT_MS);
                let advance_us = us(t0.elapsed());
                frame = frame.saturating_add(1);
                let _ = tx.send(MoodSnap {
                    pad,
                    frame,
                    advance_us,
                    at_ms: started.elapsed().as_millis() as u64,
                });
            }
        }
    }
}

fn us(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn overflow_keeps_the_newest_turn() {
        let (tx, mut rx) = TurnMailbox::pair();
        let mk = |id: u64| Heard {
            turn_id: id,
            epoch: 1,
            frames: vec![AudioFrame::capture(1)],
            listen_audio_ms: 20,
            ended_at: Instant::now(),
        };
        assert!(tx.publish(mk(1)));
        assert!(tx.publish(mk(2)));
        assert!(tx.publish(mk(3)));
        let got = rx.recv().await.expect("turn");
        assert_eq!(got.turn_id, 3);
    }
}
