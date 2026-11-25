// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `Lungs::tick`, `plan_pauses`, and `write_inhale` allocate nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_breath::{
    plan_pauses, Drive, Lungs, Pause, INHALE_SAMPLES, PAUSE_CAP, TICK_ALLOC_BUDGET,
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
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn a_tick_a_plan_and_an_inhale_allocate_nothing() {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    assert_eq!(TICK_ALLOC_BUDGET, 0);
    drop(probe);

    let mut lungs = Lungs::samantha();
    let drive = Drive::rest();
    let mut noise = [0.0f32; INHALE_SAMPLES];
    let mut pauses = [Pause::marker(); PAUSE_CAP];
    let _ = lungs.tick(drive, true);
    let _ = lungs.write_inhale(&mut noise);
    let _ = plan_pauses("The kettle is on the hob.", drive, &mut pauses);

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 0..1000 {
        let speaking = i % 17 != 0;
        let frame = lungs.tick(drive, speaking);
        let n = lungs.write_inhale(&mut noise);
        let plan = plan_pauses("where is the kettle", drive, &mut pauses);
        black_box((frame, n, plan, noise[0], pauses[0]));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        TICK_ALLOC_BUDGET,
        "the lungs tick allocated"
    );
}
