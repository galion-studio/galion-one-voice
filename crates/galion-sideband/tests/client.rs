use galion_sideband::client::*;
use galion_sideband::*;
use std::sync::{Arc, Condvar, Mutex};
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

struct Flag {
    ready: Mutex<bool>,
    cv: Condvar,
}

impl Flag {
    fn new() -> Arc<Self> {
        Arc::new(Self { ready: Mutex::new(false), cv: Condvar::new() })
    }

    fn set(&self) {
        *self.ready.lock().unwrap() = true;
        self.cv.notify_one();
    }

    fn wait(&self) {
        let mut ready = self.ready.lock().unwrap();
        while !*ready {
            ready = self.cv.wait(ready).unwrap();
        }
    }

    fn wait_until(&self, limit: Duration) {
        let mut ready = self.ready.lock().unwrap();
        let deadline = Instant::now() + limit;
        while !*ready {
            let now = Instant::now();
            assert!(now < deadline, "worker did not enter exchange");
            let (next, timeout) = self.cv.wait_timeout(ready, deadline.saturating_duration_since(now)).unwrap();
            ready = next;
            if timeout.timed_out() && !*ready {
                panic!("worker did not enter exchange");
            }
        }
    }
}

/// Parks inside `exchange` until released. The queue slot is already free
/// when `entered` is set, because the worker pops before calling the transport.
struct Parked {
    entered: Arc<Flag>,
    release: Arc<Flag>,
}

impl HermesTransport for Parked {
    fn exchange(&mut self, _: ToHermes) -> Option<FromHermes> {
        self.entered.set();
        self.release.wait();
        None
    }
}

/// Unparks the worker even when an assertion fails, so `SidebandClient`'s
/// join cannot hang. Declared after the client: locals drop in reverse order.
struct ReleaseOnDrop(Arc<Flag>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.set();
    }
}

#[test]
fn submit_and_poll_never_block() {
    let entered = Flag::new();
    let release = Flag::new();
    let mut c = SidebandClient::spawn(
        Parked { entered: Arc::clone(&entered), release: Arc::clone(&release) },
        1,
    );
    let _release = ReleaseOnDrop(release);

    assert_eq!(c.submit(lookup(1)), Ok(()));
    entered.wait_until(Duration::from_secs(2));

    // The worker is inside the first exchange, so the single slot is free.
    // Filling it, polling, and the following full-queue submit must all
    // return without waiting for that park to end. A sleep on the worker
    // cannot pin this down: the worker can pop the slot between the two
    // asserts, which is the race that failed gates on 1941d13 and 75e53e76.
    let t0 = Instant::now();
    let filled = c.submit(lookup(2));
    let polled = c.poll(TurnId(2));
    let blocked = c.submit(lookup(3));
    let elapsed = t0.elapsed();

    assert_eq!(filled, Ok(()));
    assert!(polled.is_none(), "a reply arrived while the transport was parked");
    assert_eq!(blocked, Err(SubmitError::QueueFull));
    assert!(elapsed < Duration::from_millis(80), "hot path blocked: {elapsed:?}");
}

#[test]
fn unknown_turn_is_none() {
    let mut c = SidebandClient::spawn(NullTransport, 2);
    assert!(c.poll(TurnId(42)).is_none());
}
