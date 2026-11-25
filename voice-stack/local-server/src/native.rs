// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The 7881 turn, driven by [`horbm_voice::VoiceLoop`].
//!
//! One tick is one conductor frame. The host pushes an ear event, then calls
//! [`VoiceLoop::tick`]. The brain is [`crate::brain::HorbmMind`], which calls
//! `horbm::Brain::turn`. The mouth is whatever the boot selected. This module
//! does not open a socket and does not read a wall clock.
//!
//! Boot, while the `[boot]` lock table is not on this branch: `brain` is
//! `horbm` and `mouth_device` is `cpu`. Those match the open boot lock.
//! A GPU flag does not change them. An explicit cuda mouth is the existing
//! clone opt-in, not the default.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use horbm_voice::{
    Ear, EarEvent, FakeEar, FakeMouth, Mouth, SpokenLine, TickOut, TurnBrain, VoiceLoop,
};

use crate::brain::{knowledge_dir, HorbmMind};
use crate::reply::{may_speak_piper, Pad, ReplyEngine, TurnOut};

/// `brain` for every boot of this process.
pub const BOOT_BRAIN: &str = "horbm";
/// Mouth device unless the caller explicitly opts into the clone worker.
pub const BOOT_MOUTH_DEVICE: &str = "cpu";

pub(crate) enum Stored {
    Out(TurnOut),
    Err(String),
}

pub(crate) struct MindBrain {
    pub mind: Arc<Mutex<HorbmMind>>,
    pub last: Arc<Mutex<Option<Stored>>>,
    pub turns: Arc<AtomicU32>,
}

impl MindBrain {
    pub(crate) fn new(mind: HorbmMind) -> Self {
        Self {
            mind: Arc::new(Mutex::new(mind)),
            last: Arc::new(Mutex::new(None)),
            turns: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl TurnBrain for MindBrain {
    fn turn(&mut self, heard: &str) -> SpokenLine {
        self.turns.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let result = self
            .mind
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .turn(heard);
        match result {
            Err(code) => {
                *self.last.lock().unwrap_or_else(|err| err.into_inner()) = Some(Stored::Err(code));
                empty_line()
            }
            Ok(mut out) => {
                if out.took_us == 0 {
                    out.took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
                }
                out.mood = out.mood.finite_or_resting();
                let speak = may_speak_piper(&out.reply, out.violations);
                let line = spoken_from(&out, speak);
                *self.last.lock().unwrap_or_else(|err| err.into_inner()) = Some(Stored::Out(out));
                line
            }
        }
    }
}

fn spoken_from(out: &TurnOut, speak: bool) -> SpokenLine {
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

fn empty_line() -> SpokenLine {
    SpokenLine {
        text: String::new(),
        mood: horbm::Pad::RESTING,
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    }
}

/// Ear for the socket. The host queues one transcript, then ticks.
#[derive(Default)]
pub(crate) struct QueueEar {
    pending: VecDeque<EarEvent>,
}

impl QueueEar {
    pub(crate) fn push(&mut self, event: EarEvent) {
        self.pending.push_back(event);
    }
}

impl Ear for QueueEar {
    fn id(&self) -> &str {
        "host"
    }

    fn poll(&mut self, _pcm: &[i16]) -> EarEvent {
        self.pending.pop_front().unwrap_or(EarEvent::Quiet)
    }
}

/// Local pad copied from the line the loop is about to speak.
pub(crate) fn cue_pad(mood: horbm::Pad) -> Pad {
    Pad {
        arousal: mood.arousal,
        valence: mood.valence,
        dominance: mood.dominance,
    }
}

/// One scripted turn on the conductor clock. Fake ear, fake mouth, real brain.
pub struct NativeSession {
    voice: VoiceLoop<FakeEar, FakeMouth, MindBrain>,
}

impl NativeSession {
    /// Cards from the same directory the 7881 process opens.
    pub fn scripted(events: impl IntoIterator<Item = EarEvent>) -> Self {
        let brain = MindBrain::new(HorbmMind::open(&knowledge_dir()));
        Self {
            voice: VoiceLoop::new(FakeEar::script(events), FakeMouth::new(), brain),
        }
    }

    pub fn tick(&mut self) -> TickOut {
        self.voice.tick(&[])
    }

    pub fn epoch(&self) -> u64 {
        self.voice.epoch()
    }

    pub fn barge_in_ms(&self) -> Option<u64> {
        self.voice.barge_in_ms()
    }

    pub fn cancels(&self) -> u32 {
        self.voice.mouth().cancels()
    }

    pub fn brain(&self) -> &'static str {
        BOOT_BRAIN
    }

    pub fn mouth_device(&self) -> &'static str {
        BOOT_MOUTH_DEVICE
    }

    pub fn mouth_id(&self) -> &str {
        self.voice.mouth().id()
    }
}
