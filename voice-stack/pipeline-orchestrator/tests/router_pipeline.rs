// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A finished ASR turn goes router, then the reply builder, then the mouth.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use galion_sam_horbm::{
    GapReason, LoadedCard, MemoryCards, ReasoningRouter, ResearchAnswer, ResearchPort,
    RESEARCH_CAP, UNKNOWN_REPLY,
};
use galion_sam_orch::clause::clauses;
use galion_sam_orch::clock::VAD_HANG_MS;
use galion_sam_orch::demo::{send_frames, wait_report};
use galion_sam_orch::samantha_traits::{
    mic_channel, EnergyVad, FakeAsr, FakeTts, HoldMood, MemorySpeaker, Pad,
};
use galion_sam_orch::{asr_to_piper_lines, Pipeline, RouterMouth};

struct Rec {
    calls: Arc<AtomicU32>,
}

impl ResearchPort for Rec {
    fn ask(&mut self, _question: &str, cap: Duration) -> ResearchAnswer {
        assert_eq!(cap, RESEARCH_CAP);
        self.calls.fetch_add(1, Ordering::SeqCst);
        ResearchAnswer::Answer {
            text: "A model guess.".into(),
            source: "local".into(),
        }
    }
}

fn kettle_router(calls: Arc<AtomicU32>) -> ReasoningRouter {
    let card = LoadedCard::new(
        "kettle",
        "The kettle",
        "The kettle is on the hob.",
        "Why the kettle is on the hob.",
        Vec::new(),
    )
    .unwrap();
    ReasoningRouter::new(
        MemoryCards::from_cards(vec![card]),
        Some(Box::new(Rec { calls })),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finished_asr_card_goes_to_the_mouth() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut lines_router = kettle_router(Arc::clone(&calls));
    let lines = asr_to_piper_lines(&mut lines_router, "why kettle?");
    assert_eq!(lines, vec!["The kettle is on the hob.".to_string()]);
    assert!(lines_router.gaps().is_empty());

    let calls = Arc::new(AtomicU32::new(0));
    let mouth = RouterMouth::new(kettle_router(Arc::clone(&calls)));
    let tts = FakeTts::pcm(vec![1, 2, 3, 4]);
    let texts = tts.texts.clone();
    let speaker = MemorySpeaker::new();
    let (tx, mic) = mic_channel(64);
    let handle = Pipeline {
        mic,
        vad: EnergyVad::new(VAD_HANG_MS),
        asr: FakeAsr::new("why kettle?"),
        reply: mouth,
        tts,
        speaker: speaker.clone(),
        mood: HoldMood::new(Pad::RESTING),
    }
    .spawn();
    send_frames(&tx, 6, 10).await;
    let report = wait_report(&handle, false).await;
    assert!(report.spoken);
    assert_eq!(
        texts.lock().unwrap().clone(),
        clauses("The kettle is on the hob.")
    );
    assert!(speaker.played_len() > 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finished_asr_miss_says_it_does_not_know_and_does_not_guess() {
    let calls = Arc::new(AtomicU32::new(0));
    let mouth = RouterMouth::new(kettle_router(Arc::clone(&calls)));
    let tts = FakeTts::pcm(vec![9]);
    let texts = tts.texts.clone();
    let (tx, mic) = mic_channel(64);
    let handle = Pipeline {
        mic,
        vad: EnergyVad::new(VAD_HANG_MS),
        asr: FakeAsr::new("where is the spare fuse?"),
        reply: mouth,
        tts,
        speaker: MemorySpeaker::new(),
        mood: HoldMood::default(),
    }
    .spawn();
    send_frames(&tx, 6, 10).await;
    let report = wait_report(&handle, false).await;
    assert!(report.spoken);
    let spoken = texts.lock().unwrap().clone();
    assert_eq!(spoken, clauses(UNKNOWN_REPLY));
    assert_eq!(spoken, vec![UNKNOWN_REPLY.to_string()]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    handle.shutdown().await;
}

#[test]
fn miss_on_the_router_is_queued_without_a_model_source() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut router = kettle_router(Arc::clone(&calls));
    let lines = asr_to_piper_lines(&mut router, "where is the spare fuse?");
    assert_eq!(lines, vec![UNKNOWN_REPLY.to_string()]);
    assert_eq!(router.gaps().len(), 1);
    assert_eq!(router.gaps()[0].reason, GapReason::NoCard);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
