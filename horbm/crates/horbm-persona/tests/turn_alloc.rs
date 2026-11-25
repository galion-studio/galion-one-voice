// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Verdict and the style hook allocate nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_persona::{LegalMovePort, MoveClaim, Persona, TURN_ALLOC_BUDGET};

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
        System.dealloc(ptr, layout);
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

const LOADED: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "sample"
voice = "sample"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

#[test]
#[allow(clippy::vec_init_then_push)] // the counter must see a push, not an empty Vec
fn verdict_and_style_allocate_nothing() {
    ARMED.with(|armed| armed.set(true));
    let mut probe = Vec::<u8>::new();
    probe.push(1);
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    assert_eq!(TURN_ALLOC_BUDGET, 0);
    drop(probe);

    let standin = Persona::standin();
    let loaded = Persona::parse(LOADED).unwrap();
    let claim = MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card: false,
        sympathy_script: false,
    };

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for _ in 0..1000 {
        let port: &dyn LegalMovePort = &standin;
        let verdict = port.verdict(claim);
        let style = standin.style();
        let warmth = standin.trait_value("warmth");
        let forbids = standin.forbids(b"unsourced-fact");
        let line = standin.unknown_line();
        let baseline = loaded.baseline();
        let loaded_verdict = loaded.verdict(claim);
        let loaded_style = loaded.style();
        black_box((
            verdict,
            style,
            warmth,
            forbids,
            line,
            baseline,
            loaded_verdict,
            loaded_style,
        ));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        TURN_ALLOC_BUDGET,
        "persona turn path allocated"
    );
}
