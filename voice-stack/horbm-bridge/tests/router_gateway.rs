// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `route` keeps cards, then the localhost research gateway.
//! `ReplyEngine::turn` speaks the brain's local clause. A deep card miss
//! is `I don't know.` The gateway answer is a follow-up, not that reply.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use galion_research_gateway::{LocalAnswer, Lookup, ResearchClient, Server, RESEARCH_CAP};
use galion_sam_horbm::{
    GapReason, LoadedCard, MemoryCards, ReasoningRouter, ReplyEngine, ResearchAnswer, ResearchPort,
    RouteKind, UNKNOWN_REPLY,
};

struct Counting {
    calls: Arc<AtomicUsize>,
}

impl Lookup for Counting {
    fn lookup(&self, question: &str) -> Option<LocalAnswer> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if question.contains("fuse") {
            Some(LocalAnswer {
                text: "The hob is warm.".into(),
                source: "local".into(),
            })
        } else {
            None
        }
    }
}

fn router(client: ResearchClient) -> ReasoningRouter {
    let card = LoadedCard::new(
        "kettle",
        "The kettle",
        "The kettle is on the hob.",
        "Why the kettle is on the hob.",
        Vec::new(),
    )
    .unwrap();
    let port: Box<dyn ResearchPort> = Box::new(client);
    ReasoningRouter::new(MemoryCards::from_cards(vec![card]), Some(port))
}

#[test]
fn router_order_against_a_live_localhost_gateway() {
    let calls = Arc::new(AtomicUsize::new(0));
    let server = Server::spawn(
        "127.0.0.1:0",
        Arc::new(Counting {
            calls: Arc::clone(&calls),
        }),
        RESEARCH_CAP,
    )
    .expect("127.0.0.1 bind");
    assert_eq!(server.addr().ip().to_string(), "127.0.0.1");
    let client = ResearchClient::connect(&server.addr().to_string()).unwrap();
    let mut router = router(client);

    let card = router.route("why kettle?");
    assert_eq!(card.kind, RouteKind::Card);
    assert_eq!(card.spoken, "The kettle is on the hob.");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let deep = router.route("why is the spare fuse warm?");
    assert_eq!(deep.kind, RouteKind::Research);
    assert_eq!(deep.spoken, "The hob is warm.");
    assert_eq!(deep.sources, vec!["research:local".to_string()]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        router.gaps().last().unwrap().reason,
        GapReason::ResearchAnswered
    );

    let shallow = router.route("where is the spare fuse?");
    assert_eq!(shallow.kind, RouteKind::Unknown);
    assert_eq!(shallow.spoken, UNKNOWN_REPLY);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(router.gaps().last().unwrap().reason, GapReason::NoCard);
}

const DEEP: &str = "why is the spare fuse missing from the drawer?";
const GATEWAY: &str = "The hob is warm.";

struct Script {
    calls: Arc<AtomicU32>,
    answer: ResearchAnswer,
}

impl ResearchPort for Script {
    fn ask(&mut self, _question: &str, _cap: Duration) -> ResearchAnswer {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.answer.clone()
    }
}

/// Records the ask only after `release` arrives, so a caller can see that
/// the reply exists before any ask is counted.
struct Gated {
    calls: Arc<AtomicU32>,
    caps: Arc<Mutex<Vec<Duration>>>,
    release: std::sync::mpsc::Receiver<()>,
    answer: ResearchAnswer,
}

impl ResearchPort for Gated {
    fn ask(&mut self, _question: &str, cap: Duration) -> ResearchAnswer {
        let _ = self.release.recv_timeout(Duration::from_secs(5));
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.caps.lock().expect("caps").push(cap);
        self.answer.clone()
    }
}

fn kettle_card() -> LoadedCard {
    LoadedCard::new(
        "kettle",
        "The kettle",
        "The kettle is on the hob.",
        "Why the kettle is on the hob.",
        Vec::new(),
    )
    .unwrap()
}

fn scripted(answer: ResearchAnswer) -> (ReasoningRouter, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let router = ReasoningRouter::new(
        MemoryCards::from_cards(vec![kettle_card()]),
        Some(Box::new(Script {
            calls: Arc::clone(&calls),
            answer,
        })),
    );
    (router, calls)
}

fn gateway_answer() -> ResearchAnswer {
    ResearchAnswer::Answer {
        text: GATEWAY.into(),
        source: "local".into(),
    }
}

#[test]
fn router_turn_speaks_i_dont_know_on_a_deep_card_miss() {
    let (mut router, _) = scripted(gateway_answer());
    let reply = router.turn(DEEP);
    assert_eq!(reply.text, "I don't know.");
    assert_eq!(reply.text, UNKNOWN_REPLY);
    assert_ne!(reply.text, GATEWAY);
    assert!(reply
        .sources
        .iter()
        .all(|source| !source.contains("research:")));
}

#[test]
fn router_turn_does_not_ask_research_before_the_reply_exists() {
    let calls = Arc::new(AtomicU32::new(0));
    let caps = Arc::new(Mutex::new(Vec::new()));
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut router = ReasoningRouter::new(
        MemoryCards::from_cards(vec![kettle_card()]),
        Some(Box::new(Gated {
            calls: Arc::clone(&calls),
            caps: Arc::clone(&caps),
            release: release_rx,
            answer: gateway_answer(),
        })),
    );

    let reply = router.turn(DEEP);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(reply.text, "I don't know.");
    assert_ne!(reply.text, GATEWAY);
    thread::sleep(Duration::from_millis(40));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the stand-in recorded an ask before it was released"
    );

    release_tx.send(()).expect("release");
    let started = Instant::now();
    let follow = loop {
        if let Some(follow) = router.research_follow_up() {
            break follow;
        }
        if started.elapsed() > Duration::from_secs(3) {
            panic!("research follow-up did not arrive");
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(caps.lock().expect("caps").as_slice(), &[RESEARCH_CAP]);
    assert_eq!(reply.text, "I don't know.");
    assert_eq!(follow.spoken.as_deref(), Some(GATEWAY));
    assert_eq!(follow.sources, vec!["research:local".to_string()]);
    assert!(follow.heard.contains("fuse"));
}

#[test]
fn router_turn_card_hit_matches_route() {
    let (mut router, calls) = scripted(gateway_answer());
    let heard = "why kettle?";
    let reply = router.turn(heard);
    let outcome = router.route(heard);
    assert_eq!(reply.text, outcome.spoken);
    assert_eq!(reply.text, "The kettle is on the hob.");
    assert_eq!(reply.sources, outcome.sources);
    assert_eq!(outcome.kind, RouteKind::Card);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn route_is_unchanged_for_the_orchestrator() {
    let (mut router, calls) = scripted(gateway_answer());

    let card = router.route("why kettle?");
    assert_eq!(card.kind, RouteKind::Card);
    assert_eq!(card.spoken, "The kettle is on the hob.");
    assert!(card.gap.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(router.gaps().is_empty());

    let deep = router.route("why is the spare fuse warm?");
    assert_eq!(deep.kind, RouteKind::Research);
    assert_eq!(deep.spoken, GATEWAY);
    assert_eq!(deep.sources, vec!["research:local".to_string()]);
    assert_eq!(
        router.gaps().last().unwrap().reason,
        GapReason::ResearchAnswered
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let shallow = router.route("where is the spare fuse?");
    assert_eq!(shallow.kind, RouteKind::Unknown);
    assert_eq!(shallow.spoken, UNKNOWN_REPLY);
    assert_eq!(router.gaps().last().unwrap().reason, GapReason::NoCard);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
