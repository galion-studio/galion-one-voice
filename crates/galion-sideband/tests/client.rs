use galion_sideband::client::*;
use galion_sideband::*;
use std::time::{Duration, Instant};

fn lookup(t: u64) -> ToHermes { ToHermes::Intent { turn: TurnId(t), intent: Intent::Lookup { query: "x".into() } } }

#[test]
fn null_transport_gives_honest_unreachable() {
    let mut c = SidebandClient::spawn(NullTransport, 8);
    c.submit(lookup(1)).unwrap();
    let t0 = Instant::now();
    loop {
        if let Some(r) = c.poll(TurnId(1)) {
            assert_eq!(r, FromHermes::NoKnowledge { turn: TurnId(1), reason: NoKnowledgeReason::Unreachable });
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
}

struct Slow;
impl HermesTransport for Slow {
    fn exchange(&mut self, _: ToHermes) -> Option<FromHermes> { std::thread::sleep(Duration::from_millis(200)); None }
}

#[test]
fn submit_and_poll_never_block() {
    let mut c = SidebandClient::spawn(Slow, 1);
    let t0 = Instant::now();
    for i in 0..5 { let _ = c.submit(lookup(i)); let _ = c.poll(TurnId(i)); }
    assert!(t0.elapsed() < Duration::from_millis(80), "hot path blocked: {:?}", t0.elapsed());
    assert_eq!(c.submit(lookup(9)), Err(SubmitError::QueueFull));
}

#[test]
fn unknown_turn_is_none() {
    let mut c = SidebandClient::spawn(NullTransport, 2);
    assert!(c.poll(TurnId(42)).is_none());
}
