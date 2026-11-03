// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! `NerveEmitter::note` runs on the frame host, after `Heartbeat::step`.
//! It must not allocate. JSON and I/O stay on the `voice-nerve` thread,
//! where this counter is not armed.
use galion_openmind::{NerveEmitter, VoiceNote};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

#[test]
fn note_allocates_nothing_on_the_caller() {
    let em = NerveEmitter::local(32);
    em.note(VoiceNote::frame_miss(0)).unwrap();
    let t0 = Instant::now();
    while em.stats().published == 0 {
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    ARMED.with(|a| a.set(true));
    let probe = std::hint::black_box(Box::new(1u64));
    ARMED.with(|a| a.set(false));
    drop(probe);
    assert!(
        ALLOCS.swap(0, Ordering::SeqCst) > 0,
        "counting allocator is live"
    );
    ARMED.with(|a| a.set(true));
    for i in 1..9 {
        em.note(VoiceNote::frame_miss(i)).unwrap();
    }
    ARMED.with(|a| a.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "NerveEmitter::note allocated"
    );
}
