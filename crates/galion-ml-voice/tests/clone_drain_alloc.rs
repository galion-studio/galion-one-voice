// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `CloneFrameDrain::drain_frame` allocates nothing and does not generate.
#![cfg(feature = "speech")]

use galion_ml_voice::speech::clone_worker::{CloneFrameDrain, CLONE_FRAME_SAMPLES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }

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
static ALLOC: Counting = Counting;

#[test]
fn drain_frame_allocates_nothing_and_does_not_generate() {
    let drain = CloneFrameDrain::new();
    let samples = vec![1i16; CLONE_FRAME_SAMPLES];
    assert_eq!(drain.push_slice(&samples), CLONE_FRAME_SAMPLES);
    ARMED.with(|armed| armed.set(true));
    let probe = std::hint::black_box(Box::new(1u8));
    ARMED.with(|armed| armed.set(false));
    drop(probe);
    assert!(
        ALLOCS.swap(0, Ordering::SeqCst) > 0,
        "counting allocator is live"
    );
    let mut out = [0i16; CLONE_FRAME_SAMPLES];
    ARMED.with(|armed| armed.set(true));
    let ready = drain.drain_frame(&mut out);
    let empty = drain.drain_frame(&mut out);
    ARMED.with(|armed| armed.set(false));
    assert!(ready);
    assert!(!empty);
    assert_eq!(out[0], 1);
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "drain_frame allocated");
}
