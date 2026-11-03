// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
use galion_openmind::handoff::*;
use galion_openmind::CortexHandoff;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn offer_lookup(id: u64) -> TakeoverOffer {
    TakeoverOffer {
        turn: Turn {
            id: TurnId(id),
            speaker: Speaker::User,
            text: "what time is sunset".into(),
            started_ms: 0,
            ended_ms: Some(80),
        },
        intent: Intent::Lookup {
            query: "sunset".into(),
        },
        affect: Affect {
            valence: 0.1,
            arousal: 0.2,
            confidence: 0.4,
        },
    }
}

#[test]
fn offline_hermes_is_honest_unreachable() {
    let mut h = CortexHandoff::local();
    let r = h.offer(offer_lookup(1));
    assert_eq!(r.intent, IntentDisposition::Sent);
    assert!(r.turn_sent && r.affect_sent);
    let t0 = Instant::now();
    loop {
        if let Some(reply) = h.poll(TurnId(1)) {
            assert_eq!(
                reply,
                FromHermes::NoKnowledge {
                    turn: TurnId(1),
                    reason: NoKnowledgeReason::Unreachable
                }
            );
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
}

#[test]
fn money_is_not_offered() {
    let mut h = CortexHandoff::local();
    let r = h.offer(TakeoverOffer {
        turn: Turn {
            id: TurnId(7),
            speaker: Speaker::User,
            text: "send the payment".into(),
            started_ms: 0,
            ended_ms: None,
        },
        intent: Intent::Action {
            name: "wire".into(),
            tier: ActionTier::Money,
        },
        affect: Affect {
            valence: 0.0,
            arousal: 0.0,
            confidence: 1.0,
        },
    });
    assert_eq!(r.intent, IntentDisposition::RefusedMoney);
    assert!(h.poll(TurnId(7)).is_none());
}

struct Slow;
impl HermesTransport for Slow {
    fn exchange(&mut self, _: ToHermes) -> Option<FromHermes> {
        std::thread::sleep(Duration::from_millis(200));
        None
    }
}

#[test]
fn offer_never_blocks() {
    let mut h = CortexHandoff::spawn(Slow, 1);
    let t0 = Instant::now();
    for i in 0..6 {
        let _ = h.offer(offer_lookup(i));
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed < Duration::from_millis(80),
        "handoff blocked: {elapsed:?}"
    );
}

struct Capture(Arc<Mutex<Vec<ToHermes>>>);
impl HermesTransport for Capture {
    fn exchange(&mut self, ev: ToHermes) -> Option<FromHermes> {
        self.0.lock().unwrap().push(ev);
        None
    }
}

#[test]
fn affect_is_clamped_before_it_reaches_hermes() {
    let got = Arc::new(Mutex::new(Vec::new()));
    let mut h = CortexHandoff::spawn(Capture(got.clone()), 8);
    h.offer(TakeoverOffer {
        turn: Turn {
            id: TurnId(3),
            speaker: Speaker::Student,
            text: "local filler".into(),
            started_ms: 10,
            ended_ms: None,
        },
        intent: Intent::Chat,
        affect: Affect {
            valence: 9.0,
            arousal: -4.0,
            confidence: 3.0,
        },
    });
    let t0 = Instant::now();
    let affect = loop {
        let g = got.lock().unwrap();
        if let Some(ToHermes::Affect { affect, .. }) =
            g.iter().find(|e| matches!(e, ToHermes::Affect { .. }))
        {
            break *affect;
        }
        drop(g);
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    };
    assert_eq!(
        affect,
        Affect {
            valence: 1.0,
            arousal: -1.0,
            confidence: 1.0
        }
    );
    assert_eq!(retrieval_cap_ms(), 3_000);
}
