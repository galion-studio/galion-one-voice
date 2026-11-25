// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The local clause is returned before Galion Research is asked.
//!
//! A slow `ResearchAsk::ask` must not move the time at which `first_clause`
//! and `reply` are in hand. Deny (no port) and an unreachable port leave
//! that turn the same.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use horbm::{
    AskOutcome, Brain, CardHit, CardLookup, GapReason, ResearchAsk, ResearchFollowUp, TurnOut,
    RESEARCH_CAP, UNKNOWN_REPLY,
};

const DEEP: &str = "why is the spare fuse missing from the drawer?";
const CLAUSE_BOUND: Duration = Duration::from_millis(500);

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

struct Gate {
    calls: Arc<AtomicU32>,
    entered: Arc<Mutex<Option<Instant>>>,
    seen: Arc<Mutex<Vec<String>>>,
    sleep: Duration,
    outcome: AskOutcome,
}

impl ResearchAsk for Gate {
    fn ask(&mut self, question: &str, cap: Duration) -> AskOutcome {
        assert_eq!(cap, RESEARCH_CAP);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().expect("seen").push(question.to_string());
        *self.entered.lock().expect("entered") = Some(Instant::now());
        if !self.sleep.is_zero() {
            thread::sleep(self.sleep);
        }
        self.outcome.clone()
    }
}

fn gate(
    calls: Arc<AtomicU32>,
    entered: Arc<Mutex<Option<Instant>>>,
    sleep: Duration,
    outcome: AskOutcome,
) -> Gate {
    Gate {
        calls,
        entered,
        seen: Arc::new(Mutex::new(Vec::new())),
        sleep,
        outcome,
    }
}

fn wait_entered(slot: &Mutex<Option<Instant>>) -> Instant {
    let started = Instant::now();
    loop {
        if let Some(when) = *slot.lock().expect("entered") {
            return when;
        }
        if started.elapsed() > Duration::from_secs(2) {
            panic!("research ask did not start");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_follow(brain: &mut Brain, limit: Duration) -> ResearchFollowUp {
    let started = Instant::now();
    loop {
        if let Some(follow) = brain.research_follow_up() {
            return follow;
        }
        if started.elapsed() > limit {
            panic!("research follow-up did not arrive");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn speech(
    out: &TurnOut,
) -> (
    String,
    String,
    String,
    Vec<String>,
    Vec<String>,
    String,
    Option<bool>,
) {
    (
        out.reply.clone(),
        out.first_clause.clone(),
        out.rest.clone(),
        out.sources.clone(),
        out.evidence.clone(),
        out.arm.clone(),
        out.sideband.as_ref().map(|mark| mark.posted),
    )
}

#[test]
fn the_first_clause_does_not_wait_on_research() {
    let calls = Arc::new(AtomicU32::new(0));
    let entered = Arc::new(Mutex::new(None));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(gate(
            Arc::clone(&calls),
            Arc::clone(&entered),
            RESEARCH_CAP,
            AskOutcome::Answer {
                text: "The hob is warm.".into(),
                source: "local".into(),
            },
        ))),
    );

    let started = Instant::now();
    let out = brain.turn(DEEP);
    assert!(
        entered.lock().expect("entered").is_none(),
        "research started before the clause was returned"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.first_clause, UNKNOWN_REPLY);
    assert_eq!(out.rest, "");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    match out.sideband {
        Some(ref mark) => assert!(!mark.posted),
        None => panic!("a deep miss is still marked, and not posted"),
    }
    assert!(
        started.elapsed() < CLAUSE_BOUND,
        "turn waited on research: {:?}",
        started.elapsed()
    );
    let clause_ready = Instant::now();
    let reply = out.reply.clone();
    let first = out.first_clause.clone();
    drop(out);
    assert!(
        started.elapsed() < CLAUSE_BOUND,
        "dropping the clause waited on research: {:?}",
        started.elapsed()
    );

    let asked_at = wait_entered(&entered);
    assert!(
        asked_at >= clause_ready,
        "ask started before the clause was in hand"
    );

    let during = Instant::now();
    let card = brain.turn("where is the kettle");
    assert!(
        during.elapsed() < CLAUSE_BOUND,
        "a later local clause waited on the in-flight ask: {:?}",
        during.elapsed()
    );
    assert_eq!(card.first_clause, "The kettle");
    assert!(card.first_clause.split_whitespace().count() <= 2);
    assert_eq!(card.reply, "The kettle is on the hob.");
    assert_eq!(card.rest, "is on the hob.");
    assert_eq!(card.arm, "answer");
    assert_eq!(card.sources, vec!["card:kettle".to_string()]);
    drop(card);

    let follow = wait_follow(&mut brain, Duration::from_secs(6));
    assert!(
        started.elapsed() >= Duration::from_millis(2_900),
        "the cap was not held off the clause: {:?}",
        started.elapsed()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(reply, UNKNOWN_REPLY);
    assert_eq!(first, UNKNOWN_REPLY);
    assert_eq!(follow.heard, DEEP);
    assert_eq!(follow.spoken.as_deref(), Some("The hob is warm."));
    assert_eq!(follow.sources, vec!["research:local".to_string()]);
    assert_eq!(follow.gap.reason, GapReason::ResearchAnswered);
    assert!(follow.gap.heard.contains("fuse"));
}

#[test]
fn a_local_hit_and_comfort_never_start_research() {
    let calls = Arc::new(AtomicU32::new(0));
    let entered = Arc::new(Mutex::new(None));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(gate(
            Arc::clone(&calls),
            Arc::clone(&entered),
            Duration::ZERO,
            AskOutcome::Answer {
                text: "A model guess.".into(),
                source: "local".into(),
            },
        ))),
    );
    let card = brain.turn("why is the kettle on the hob?");
    assert_eq!(card.first_clause, "The kettle");
    assert_eq!(card.reply, "The kettle is on the hob.");
    assert_eq!(card.arm, "answer");
    drop(card);
    let comfort = brain.turn("why am I so sad?");
    assert_eq!(comfort.reply, UNKNOWN_REPLY);
    assert_eq!(comfort.first_clause, UNKNOWN_REPLY);
    assert_eq!(brain.dialogue_lean().verb, horbm::Verb::Comfort);
    drop(comfort);
    thread::sleep(Duration::from_millis(40));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(entered.lock().expect("entered").is_none());
    assert!(brain.research_follow_up().is_none());
}

#[test]
fn research_deny_or_offline_leaves_the_turn_unchanged() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut denied = Brain::with_cards(Cards, None);
    let mut offline = Brain::with_cards(
        Cards,
        Some(Box::new(Gate {
            calls: Arc::clone(&calls),
            entered: Arc::new(Mutex::new(None)),
            seen: Arc::new(Mutex::new(Vec::new())),
            sleep: Duration::ZERO,
            outcome: AskOutcome::Unreachable,
        })),
    );

    for line in ["where is the kettle", "I am so tired", "why am I so sad?"] {
        let left = denied.turn(line);
        let right = offline.turn(line);
        assert_eq!(speech(&left), speech(&right), "{line}");
        if line.contains("kettle") {
            assert_eq!(left.first_clause, "The kettle");
            assert_eq!(left.reply, "The kettle is on the hob.");
        }
        drop(left);
        drop(right);
    }
    thread::sleep(Duration::from_millis(40));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let left = denied.turn(DEEP);
    let right = offline.turn(DEEP);
    assert_eq!(speech(&left), speech(&right));
    assert_eq!(left.reply, UNKNOWN_REPLY);
    assert_eq!(left.first_clause, UNKNOWN_REPLY);
    assert_eq!(left.arm, "ask");
    assert_eq!(denied.gaps().last().unwrap().reason, GapReason::ResearchOff);
    assert_eq!(
        offline.gaps().last().unwrap().reason,
        GapReason::ResearchOff
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let reply = left.reply.clone();
    let first = left.first_clause.clone();
    drop(left);
    drop(right);

    let follow = wait_follow(&mut offline, Duration::from_secs(2));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(follow.outcome, AskOutcome::Unreachable);
    assert!(follow.spoken.is_none());
    assert_eq!(follow.gap.reason, GapReason::ResearchUnreachable);
    assert_eq!(reply, UNKNOWN_REPLY);
    assert_eq!(first, UNKNOWN_REPLY);
    assert_eq!(
        offline.gaps().last().unwrap().reason,
        GapReason::ResearchUnreachable
    );
    assert_eq!(denied.gaps().last().unwrap().reason, GapReason::ResearchOff);
}
