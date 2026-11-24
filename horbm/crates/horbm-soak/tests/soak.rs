// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Counting allocator for the duplex soak. The CI run is five virtual
//! minutes. `GALION_SOAK_FULL=1` runs sixty.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_soak::{minutes_from_env, run};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed) as u64
}

#[test]
fn samantha_duplex_long_call() {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8; 8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::Relaxed) > 0,
        "the counter must see a Vec"
    );
    drop(probe);
    ALLOCS.store(0, Ordering::Relaxed);
    ARMED.with(|armed| armed.set(true));

    let minutes = minutes_from_env(std::env::var("GALION_SOAK_FULL").ok().as_deref());
    let report = run(minutes, &allocs);
    println!("{}", report.table());
    println!("{}", report.stages());
    ARMED.with(|armed| armed.set(false));
    report.enforce().expect("soak gate");
}
