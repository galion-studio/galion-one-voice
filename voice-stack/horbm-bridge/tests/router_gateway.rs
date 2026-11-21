// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Cards first, then the localhost research gateway. No guess on a miss.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use galion_research_gateway::{LocalAnswer, Lookup, ResearchClient, Server, RESEARCH_CAP};
use galion_sam_horbm::{
    GapReason, LoadedCard, MemoryCards, ReasoningRouter, ResearchPort, RouteKind, UNKNOWN_REPLY,
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
