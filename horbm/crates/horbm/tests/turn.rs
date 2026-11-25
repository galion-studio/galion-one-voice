// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Full turn: mock cards, mock gateway, empathy mood on the reply.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use horbm::{
    AskOutcome, Brain, CardHit, CardLookup, GapReason, ResearchAsk, ResearchFollowUp, RESEARCH_CAP,
    UNKNOWN_REPLY,
};

struct Cards;

impl CardLookup for Cards {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

struct Gateway {
    calls: Arc<AtomicU32>,
    caps: Arc<Mutex<Vec<Duration>>>,
    sleep_cap: bool,
    answer: AskOutcome,
}

impl ResearchAsk for Gateway {
    fn ask(&mut self, _question: &str, cap: Duration) -> AskOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.caps.lock().expect("caps").push(cap);
        if self.sleep_cap {
            thread::sleep(cap);
        }
        self.answer.clone()
    }
}

#[test]
fn miss_is_exact_and_mood_passes_through_a_card() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
            caps: Arc::new(Mutex::new(Vec::new())),
            sleep_cap: false,
            answer: AskOutcome::Answer {
                text: "A filler line.".into(),
                source: "local".into(),
            },
        })),
    );
    let miss = brain.turn("I am so tired");
    assert_eq!(miss.reply, UNKNOWN_REPLY);
    assert_eq!(miss.reply, "I don't know.");
    assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
    assert!(miss.evidence.is_empty());
    assert!(miss.mood_meta.e > 0.0);
    assert!(miss.mood.valence > 0.0);
    assert_eq!(miss.mood.arousal, miss.mood_meta.arousal);
    assert_eq!(miss.mood.valence, miss.mood_meta.valence);
    assert_eq!(miss.mood.dominance, miss.mood_meta.dominance);
    assert!(miss.mood_meta.named.is_none());
    assert!(!miss.reply.to_ascii_lowercase().contains("sorry"));
    assert!(!miss.reply.to_ascii_lowercase().contains("filler"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(brain.gaps()[0].reason, GapReason::NoCard);
    assert_eq!(brain.gaps()[0].advice, "ask_galion_research");

    let hit = brain.turn("I am so tired where is the kettle");
    assert_eq!(hit.reply, "The kettle is on the hob.");
    assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
    assert!(hit.mood_meta.e > 0.0);
    assert!(hit.mood_meta.rate.is_finite());
    assert!(hit.mood_meta.energy.is_finite());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn gateway_timeout_at_three_seconds_falls_back() {
    let calls = Arc::new(AtomicU32::new(0));
    let caps = Arc::new(Mutex::new(Vec::new()));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
            caps: Arc::clone(&caps),
            sleep_cap: false,
            answer: AskOutcome::Timeout,
        })),
    );
    let started = Instant::now();
    let out = brain.turn("why is the spare fuse missing from the drawer?");
    let elapsed = started.elapsed();
    eprintln!("MEASURE horbm_gateway_timeout_ms={}", elapsed.as_millis());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "research ran inside the turn"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "the clause waited on research: {elapsed:?}"
    );
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.first_clause, "I don't know.");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(!out.reply.to_ascii_lowercase().contains("ollama"));
    assert!(out.mood_meta.intent == "ask" || out.mood_meta.intent == "chat");
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchOff);
    drop(out);
    let follow = wait_follow(&mut brain);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
    assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
    assert_eq!(follow.outcome, AskOutcome::Timeout);
    assert!(follow.spoken.is_none());
    assert_eq!(follow.gap.reason, GapReason::ResearchTimeout);
    assert_eq!(
        brain.gaps().last().unwrap().reason,
        GapReason::ResearchTimeout
    );
}

fn wait_follow(brain: &mut Brain) -> ResearchFollowUp {
    let started = Instant::now();
    loop {
        if let Some(follow) = brain.research_follow_up() {
            return follow;
        }
        if started.elapsed() > Duration::from_secs(2) {
            panic!("research follow-up did not arrive");
        }
        thread::sleep(Duration::from_millis(5));
    }
}
