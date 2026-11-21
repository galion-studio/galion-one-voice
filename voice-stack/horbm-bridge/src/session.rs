// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The 80 ms mood clock around a [`ReplyEngine`].
//!
//! [`Session::tick`] is the call a host already inside `galion-heartbeat`
//! makes once per frame, beside `Heartbeat::step`, not inside it. The
//! heartbeat step does not allocate and does not lock. This tick is one
//! call. It does not start a turn, read a card, or open a socket.
//! [`crate::CardEngine`] may ease the pad toward rest. [`crate::FakeEngine`]
//! holds the pad.
//!
//! [`MoodTick`] is the clock for a host that has no heartbeat thread yet.
//! It sleeps until the next frame, ticks once, and if it wakes late it does
//! not catch up. That is the heartbeat rule: a late frame is a pause.
//! A `turn` holds the same lock, so a slow brain delays the next tick and
//! still does not burst.

use std::io;
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::chunks::speakable_chunks;
use crate::engine::{Reply, ReplyEngine};
use crate::pad::{Pad, FRAME_MS};
use crate::SpeakChunk;

/// A reply plus the chunks the mouth can speak now.
#[derive(Clone, Debug, PartialEq)]
pub struct SpokenTurn {
    pub reply: Reply,
    pub chunks: Vec<SpeakChunk>,
}

/// Single-threaded session. The host drives the frame clock.
pub struct Session<E: ReplyEngine> {
    engine: E,
    frames: u64,
}

impl<E: ReplyEngine> Session<E> {
    pub fn new(engine: E) -> Self {
        Self { engine, frames: 0 }
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn mood(&self) -> Pad {
        self.engine.mood()
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// One 80 ms frame. Does not call [`ReplyEngine::turn`].
    pub fn tick(&mut self) -> Pad {
        self.frames = self.frames.saturating_add(1);
        self.engine.tick()
    }

    /// Ask the engine, then split the reply for streaming TTS.
    /// A turn does not consume a frame.
    pub fn turn(&mut self, text: &str) -> SpokenTurn {
        let reply = self.engine.turn(text);
        let chunks = speakable_chunks(&reply.text);
        SpokenTurn { reply, chunks }
    }
}

struct Slot<E> {
    engine: E,
    frames: u64,
}

/// Background 80 ms mood clock. Drop joins the thread.
pub struct MoodTick<E: ReplyEngine> {
    shared: std::sync::Arc<Mutex<Slot<E>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl<E: ReplyEngine + Send + 'static> MoodTick<E> {
    /// Start the clock. The first tick is one frame after spawn.
    pub fn spawn(engine: E) -> io::Result<Self> {
        let shared = std::sync::Arc::new(Mutex::new(Slot { engine, frames: 0 }));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shared_thread = std::sync::Arc::clone(&shared);
        let stop_thread = std::sync::Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("galion-sam-mood-80ms".into())
            .spawn(move || mood_loop(shared_thread, stop_thread))?;
        Ok(Self {
            shared,
            stop,
            join: Some(join),
        })
    }

    pub fn frames(&self) -> u64 {
        self.lock().frames
    }

    pub fn mood(&self) -> Pad {
        self.lock().engine.mood()
    }

    pub fn turn(&self, text: &str) -> SpokenTurn {
        let mut slot = self.lock();
        let reply = slot.engine.turn(text);
        let chunks = speakable_chunks(&reply.text);
        SpokenTurn { reply, chunks }
    }

    fn lock(&self) -> MutexGuard<'_, Slot<E>> {
        self.shared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

impl<E: ReplyEngine> Drop for MoodTick<E> {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn mood_loop<E: ReplyEngine>(
    shared: std::sync::Arc<Mutex<Slot<E>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let frame = Duration::from_millis(u64::from(FRAME_MS));
    let slice = Duration::from_millis(20);
    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        let wake = Instant::now() + frame;
        while Instant::now() < wake {
            if stop.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let left = wake.saturating_duration_since(Instant::now());
            thread::sleep(left.min(slice));
        }
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let mut slot = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        slot.engine.tick();
        slot.frames = slot.frames.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeEngine;
    use crate::pad::Pad;

    #[test]
    fn ticks_between_turns_hold_the_pad_and_a_turn_does_not_consume_a_frame() {
        let mut session = Session::new(FakeEngine::new());
        let loud = session.turn("hey!");
        assert_eq!(loud.reply.mood, Pad::new(0.7, 0.55, 0.5));
        assert_eq!(loud.chunks.len(), 1);
        assert_eq!(loud.chunks[0].text, "hey!");
        assert_eq!(session.frames(), 0);
        let before = session.mood();
        for _ in 0..125 {
            session.tick();
        }
        assert_eq!(session.frames(), 125);
        assert_eq!(session.mood(), before);
        let quiet = session.turn("the kettle is on");
        assert_eq!(session.frames(), 125);
        assert_eq!(quiet.reply.text, "the kettle is on.");
        assert_eq!(quiet.chunks.len(), 1);
        assert_eq!(quiet.reply.mood, before);
    }

    #[test]
    fn two_sentences_become_two_chunks_and_a_script_is_not_spoken() {
        let mut session = Session::new(FakeEngine::new());
        let spoken = session.turn("The kettle is on. It can wait.");
        assert_eq!(spoken.chunks.len(), 2);
        assert_eq!(spoken.reply.sources, vec!["fake:echo".to_string()]);
        let blocked = session.turn("I'm sorry.");
        assert_eq!(blocked.reply.text, "I'm sorry.");
        assert!(blocked.chunks.is_empty());
    }

    #[test]
    fn the_background_tick_runs_between_turns() {
        let tick = MoodTick::spawn(FakeEngine::new()).expect("spawn mood tick");
        thread::sleep(Duration::from_millis(500));
        let frames = tick.frames();
        assert!(
            (2u64..30).contains(&frames),
            "expected a few 80 ms frames in 500 ms, got {frames}"
        );
        let loud = tick.turn("hey!");
        assert_eq!(loud.reply.mood.arousal, 0.7);
        thread::sleep(Duration::from_millis(500));
        let later = tick.frames();
        assert!(
            later > frames,
            "tick stopped between turns: {frames} -> {later}"
        );
        assert_eq!(tick.mood().arousal, 0.7);
        assert!(
            (2u64..40).contains(&later),
            "frame count left the 80 ms band: {later}"
        );
    }
}
