// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The 80 ms tick must not touch the heap.

use galion_sam_prosody::{Pad, ProsodyClock};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn tick_does_not_allocate() {
    let mut clock = ProsodyClock::new();
    let live = ALLOCS.load(Ordering::SeqCst);
    drop(black_box(Box::new(0u8)));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > live,
        "counting allocator is live"
    );

    let pads = [
        Pad::RESTING,
        Pad {
            arousal: 0.9,
            valence: 0.8,
            dominance: 0.6,
        },
        Pad {
            arousal: 0.05,
            valence: -0.4,
            dominance: 0.2,
        },
        Pad {
            arousal: f32::NAN,
            valence: 0.4,
            dominance: 0.5,
        },
    ];
    let before = ALLOCS.load(Ordering::SeqCst);
    let mut acc = 0.0f32;
    for i in 0..10_000u32 {
        clock.set_target(pads[(i as usize) % pads.len()]);
        let params = clock.tick();
        acc += params.exaggeration + params.length_scale + params.pre_pause_ms;
        acc += params.tag.map(|t| t.token().len() as f32).unwrap_or(0.0);
    }
    black_box(acc);
    let after = ALLOCS.load(Ordering::SeqCst);
    assert_eq!(after, before, "tick allocated {} times", after - before);
}
