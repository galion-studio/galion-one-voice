// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `InnerLife::turn` and `InnerLife::nudge` allocate nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use galion_emotion_core::{
    EmotionCore, InnerLife, IntentMsg, IntentSource, TurnView, FRAME_MS, TURN_ALLOC_BUDGET,
};

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
fn turn_and_nudge_stay_inside_the_alloc_budget() {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    assert_eq!(TURN_ALLOC_BUDGET, 0);
    drop(probe);

    let mut inbound = IntentMsg::hold(0);
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("rush").unwrap();
    let bytes = inbound.encode().unwrap();
    let decoded = IntentMsg::decode(&bytes).unwrap();

    let mut life = InnerLife::seeded();
    let core = EmotionCore::at_rest();
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 0..200u32 {
        let turn = life.turn(TurnView {
            t_ms: i * FRAME_MS,
            dialogue: b"what time is it?",
            point: core.moment().pos,
            inbound: Some(&decoded),
            consent: 0,
        });
        let wire = turn.outbound.encode().unwrap();
        let back = IntentMsg::decode(&wire).unwrap();
        black_box(back);
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        TURN_ALLOC_BUDGET,
        "InnerLife::turn allocated"
    );

    let mut life = InnerLife::seeded();
    let mut core = EmotionCore::at_rest();
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for _ in 0..200u32 {
        let turn = life.nudge(&mut core, b"pause", None).unwrap();
        black_box((turn, core.moment(), core.conductor_cue()));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        TURN_ALLOC_BUDGET,
        "InnerLife::nudge allocated"
    );
}
