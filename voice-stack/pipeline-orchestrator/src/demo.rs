// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Scripted runs on the fake stages. The binary prints these measurements.
//! The reply line is test text. It is not a persona script, and the pad is
//! not turned into extra words.

use crate::clock::{STAGE_BAR_US, VAD_HANG_MS};
use crate::pipeline::{Handle, Phase, Pipeline, TurnReport};
use crate::samantha_traits::{
    mic_channel, AudioFrame, EnergyVad, FakeAsr, FakeReply, FakeTts, HoldMood, MemorySpeaker, Pad,
    WaitGate,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

pub const HEARD: &str = "hello";
pub const REPLY: &str = "I am here. Glad you came.";
pub const MOOD: Pad = Pad {
    arousal: 0.55,
    valence: 0.7,
    dominance: 0.6,
};

pub struct Scripted {
    pub report: TurnReport,
    pub phases: Vec<Phase>,
    pub texts: Vec<String>,
    pub moods: Vec<Pad>,
    pub played: Vec<i16>,
    pub mood_after: Pad,
}

pub struct Barge {
    pub barge_us: u32,
    pub played_len: usize,
    pub queued_len: usize,
    pub cancels: u32,
    pub phases: Vec<Phase>,
    pub stale: bool,
}

pub struct MoodTicks {
    pub intervals_ms: Vec<u64>,
    pub advance_us: Vec<u32>,
}

pub struct Running {
    pub tx: mpsc::Sender<AudioFrame>,
    pub handle: Handle,
    pub speaker: MemorySpeaker,
    pub texts: Arc<Mutex<Vec<String>>>,
    pub moods: Arc<Mutex<Vec<Pad>>>,
    pub calls: Arc<AtomicU32>,
}

pub fn spawn_fake(reply: FakeReply, tts: FakeTts) -> Running {
    let calls = reply.calls.clone();
    let texts = tts.texts.clone();
    let moods = tts.moods.clone();
    let speaker = MemorySpeaker::new();
    let (tx, mic) = mic_channel(64);
    let handle = Pipeline {
        mic,
        vad: EnergyVad::new(VAD_HANG_MS),
        asr: FakeAsr::new(HEARD),
        reply,
        tts,
        speaker: speaker.clone(),
        mood: HoldMood::default(),
    }
    .spawn();
    Running {
        tx,
        handle,
        speaker,
        texts,
        moods,
        calls,
    }
}

pub async fn send_frames(tx: &mpsc::Sender<AudioFrame>, loud: usize, silent: usize) {
    for _ in 0..loud {
        tx.send(AudioFrame::capture(12_000))
            .await
            .expect("mic open");
    }
    for _ in 0..silent {
        tx.send(AudioFrame::capture(0)).await.expect("mic open");
    }
}

pub async fn wait_report(handle: &Handle, stale: bool) -> TurnReport {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(report) = handle
            .reports()
            .into_iter()
            .find(|report| report.stale == stale)
        {
            return report;
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "no report stale={stale}; phases={:?} reports={:?}",
                handle.phases(),
                handle.reports()
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

pub async fn measure_scripted() -> Scripted {
    let running = spawn_fake(
        FakeReply::new(REPLY, MOOD),
        FakeTts::pcm(vec![1000, -1000, 500, -500]),
    );
    send_frames(&running.tx, 6, 10).await;
    let report = wait_report(&running.handle, false).await;
    let mut mood_rx = running.handle.subscribe().mood_receiver();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if running.handle.mood().pad == MOOD || mood_rx.borrow().pad == MOOD {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            break;
        }
        tokio::time::timeout(Duration::from_millis(200), mood_rx.changed())
            .await
            .ok();
    }
    let mood_after = running.handle.mood().pad;
    let phases = running.handle.phases();
    let texts = running.texts.lock().expect("texts").clone();
    let moods = running.moods.lock().expect("moods").clone();
    let played = running.speaker.played();
    running.handle.shutdown().await;
    Scripted {
        report,
        phases,
        texts,
        moods,
        played,
        mood_after,
    }
}

pub async fn measure_barge() -> Barge {
    let gate = WaitGate::new();
    let running = spawn_fake(
        FakeReply::new(REPLY, MOOD),
        FakeTts::gated(vec![1000, -1000, 500, -500], gate),
    );
    send_frames(&running.tx, 6, 10).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while running.speaker.played_len() == 0 || running.handle.phase() != Phase::Speaking {
        if tokio::time::Instant::now() > deadline {
            panic!(
                "did not reach speaking; phase={:?} phases={:?}",
                running.handle.phase(),
                running.handle.phases()
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    send_frames(&running.tx, 4, 0).await;
    let report = wait_report(&running.handle, true).await;
    let barge = Barge {
        barge_us: report.barge_us.unwrap_or(u32::MAX),
        played_len: running.speaker.played_len(),
        queued_len: running.speaker.queued_len(),
        cancels: running.speaker.cancels(),
        phases: running.handle.phases(),
        stale: report.stale,
    };
    running.handle.shutdown().await;
    barge
}

pub async fn measure_think_cancel() -> bool {
    let gate = WaitGate::new();
    let running = spawn_fake(
        FakeReply::new(REPLY, MOOD).with_gate(gate.clone()),
        FakeTts::pcm(vec![1, 2, 3, 4]),
    );
    send_frames(&running.tx, 6, 10).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while running.calls.load(Ordering::SeqCst) == 0 {
        if tokio::time::Instant::now() > deadline {
            panic!("reply was not entered");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    send_frames(&running.tx, 4, 0).await;
    let phase_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !running.handle.phases().contains(&Phase::Listening)
        || running
            .handle
            .phases()
            .iter()
            .filter(|p| **p == Phase::Listening)
            .count()
            < 2
    {
        if tokio::time::Instant::now() > phase_deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    gate.release.store(true, Ordering::SeqCst);
    let report = wait_report(&running.handle, true).await;
    let silent = running.speaker.played_len() == 0 && !report.spoken;
    running.handle.shutdown().await;
    silent
}

pub async fn measure_mood() -> MoodTicks {
    let running = spawn_fake(FakeReply::new(REPLY, MOOD), FakeTts::pcm(vec![1]));
    let mut mood_rx = running.handle.subscribe().mood_receiver();
    let mut snaps = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while snaps.len() < 5 {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        if tokio::time::timeout(Duration::from_millis(500), mood_rx.changed())
            .await
            .ok()
            .and_then(|r| r.ok())
            .is_none()
        {
            break;
        }
        snaps.push(*mood_rx.borrow());
    }
    let intervals_ms = snaps
        .windows(2)
        .map(|pair| pair[1].at_ms.saturating_sub(pair[0].at_ms))
        .collect();
    let advance_us = snaps.iter().map(|snap| snap.advance_us).collect();
    running.handle.shutdown().await;
    MoodTicks {
        intervals_ms,
        advance_us,
    }
}

impl Scripted {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"scripted\",\"listen_audio_ms\":{},\"asr_us\":{},\"think_us\":{},\"first_audio_us\":{},\"eot_to_audio_us\":{},\"chunks_started\":{},\"spoken\":{},\"stage_bar_us\":{},\"phases\":{}}}",
            self.report.listen_audio_ms,
            self.report.asr_us,
            self.report.think_us,
            json_opt_u32(self.report.first_audio_us),
            json_opt_u32(self.report.eot_to_audio_us),
            self.report.chunks_started,
            self.report.spoken,
            STAGE_BAR_US,
            json_phases(&self.phases)
        )
    }
}

impl Barge {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"barge\",\"barge_us\":{},\"played_len\":{},\"queued_len\":{},\"cancels\":{},\"stale\":{},\"stage_bar_us\":{}}}",
            self.barge_us,
            self.played_len,
            self.queued_len,
            self.cancels,
            self.stale,
            STAGE_BAR_US
        )
    }
}

impl MoodTicks {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"mood\",\"intervals_ms\":{},\"advance_us\":{},\"heartbeat_ms\":80}}",
            json_u64s(&self.intervals_ms),
            json_u32s(&self.advance_us)
        )
    }
}

fn json_opt_u32(value: Option<u32>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".into())
}

fn json_phases(phases: &[Phase]) -> String {
    let body = phases
        .iter()
        .map(|phase| format!("\"{}\"", phase.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn json_u64s(values: &[u64]) -> String {
    let body = values
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn json_u32s(values: &[u32]) -> String {
    let body = values
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}
