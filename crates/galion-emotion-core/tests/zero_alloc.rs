// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `EmotionCore::step`, the conductor cue, and the local wire allocate nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use galion_emotion_core::{EmotionCore, IntentMsg, FRAME_MS, STEP_ALLOC_BUDGET};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn state_step_stays_inside_the_alloc_budget() {
    ARMED.with(|armed| armed.set(true));
    let mut probe = Vec::<u8>::new();
    probe.push(1);
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    assert_eq!(STEP_ALLOC_BUDGET, 0);
    drop(probe);

    let mut core = EmotionCore::at_rest();
    let mut msg = IntentMsg::steady(0);
    core.step(&msg).unwrap();

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 1..1000u32 {
        msg.t_ms = i * FRAME_MS;
        let moment = core.step(&msg).unwrap();
        let cue = core.conductor_cue();
        let wire = core.local_wire(moment.tick as u16);
        black_box((moment, cue, wire));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        STEP_ALLOC_BUDGET,
        "EmotionCore::step allocated"
    );
}
