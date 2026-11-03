// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Barge-in cancel against [`galion_console::turn_worker::TurnWorker::supersede`].
//!
//! This is the worker drop: a stale reply must not be delivered after `supersede`.
//! It is not an acoustic stop on a speaker, and it is not a PSTN measurement.
//! `GALION_ASR` is left alone. The probe lane is built with ASR off so the reply
//! is the fast tone path and the check does not need Piper or flite.

use std::thread;
use std::time::{Duration, Instant};

use galion_cascade::{AsrMode, LlmMode, PhoneConfig};
use galion_console::turn_worker::{TurnJob, TurnWorker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BargeProbe {
    /// `TurnWorker::supersede` was called.
    pub supersede: bool,
    pub cancel: &'static str,
    pub stale_delivered: bool,
    /// Time from `supersede` until the inbox had no deliverable stale reply.
    pub stop_ms: Option<u32>,
    pub note: String,
}

pub fn probe_supersede() -> BargeProbe {
    let worker = TurnWorker::spawn_with(PhoneConfig {
        asr: AsrMode::Off,
        llm: LlmMode::Canned,
    });
    let job = TurnJob {
        id: 1,
        rate: 16_000,
        pcm: vec![0; 1_600],
    };
    if worker.try_submit(job).is_err() {
        return BargeProbe {
            supersede: false,
            cancel: "fail",
            stale_delivered: false,
            stop_ms: None,
            note: "worker did not accept the first turn".into(),
        };
    }
    // Let the tone reply land in the inbox so supersede has something to drop.
    thread::sleep(Duration::from_millis(100));
    let t0 = Instant::now();
    worker.supersede(99);
    let mut stale = false;
    let mut stop_ms = None;
    let watch = Duration::from_millis(400);
    while t0.elapsed() < watch {
        match worker.try_recv() {
            Some(r) if r.id < 99 => {
                stale = true;
                stop_ms = Some(elapsed_ms(t0));
                break;
            }
            Some(_) => {}
            None => {
                if stop_ms.is_none() {
                    stop_ms = Some(elapsed_ms(t0));
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
    if stale {
        return BargeProbe {
            supersede: true,
            cancel: "fail",
            stale_delivered: true,
            stop_ms,
            note: "a turn older than the supersede id was delivered".into(),
        };
    }
    let follow = TurnJob {
        id: 100,
        rate: 16_000,
        pcm: vec![0; 1_600],
    };
    if worker.try_submit(follow).is_err() {
        return BargeProbe {
            supersede: true,
            cancel: "fail",
            stale_delivered: false,
            stop_ms,
            note: "worker did not accept the turn after supersede".into(),
        };
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut follow_ok = false;
    while Instant::now() < deadline {
        match worker.try_recv() {
            Some(r) if r.id < 99 => {
                return BargeProbe {
                    supersede: true,
                    cancel: "fail",
                    stale_delivered: true,
                    stop_ms,
                    note: format!(
                        "stale id {} arrived after the next turn was submitted",
                        r.id
                    ),
                };
            }
            Some(r) if r.id >= 100 => {
                follow_ok = r.backend == "tone"
                    && r.error
                        .as_deref()
                        .is_some_and(|e| e.contains("AsrUnratified"));
                break;
            }
            Some(_) => {}
            None => thread::sleep(Duration::from_millis(5)),
        }
    }
    if !follow_ok {
        return BargeProbe {
            supersede: true,
            cancel: "fail",
            stale_delivered: false,
            stop_ms,
            note: "the turn after supersede did not come back on the tone path".into(),
        };
    }
    BargeProbe {
        supersede: true,
        cancel: "pass",
        stale_delivered: false,
        stop_ms,
        note: "worker dropped the stale reply and accepted the next turn".into(),
    }
}

fn elapsed_ms(t0: Instant) -> u32 {
    t0.elapsed().as_millis().min(u32::MAX as u128) as u32
}
