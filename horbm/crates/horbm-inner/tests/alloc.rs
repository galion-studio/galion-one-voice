// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `turn`, `tick`, and `nudge` allocate nothing. JSON is not on that path.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::sync::Mutex;

use galion_emotion_core::{EmotionCore, IntentMsg, IntentSource, VALENCE};
use horbm_inner::{Heard, InnerHome, Perceived, PersonaInner, SamanthaInner, TICK_MS};

static GATE: Mutex<()> = Mutex::new(());

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
fn turn_tick_and_nudge_allocate_nothing() {
    let _hold = GATE.lock().expect("lock");
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    drop(probe);

    let mut inbound = IntentMsg::hold(0);
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("heard").unwrap();
    inbound.strength = 0.8;
    inbound.set_pull(VALENCE, -0.4).unwrap();

    let mut life = SamanthaInner::new();
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 0..100u32 {
        let turn = life.turn(Perceived {
            t_ms: i * TICK_MS,
            words: "I feel really sad",
            ear: Some(Heard {
                valence: -0.4,
                arousal: 0.3,
                confidence: 0.8,
            }),
            sense: false,
        });
        let wire = turn.to_gemi().unwrap();
        black_box(wire);
        let quiet = life.tick(TICK_MS, true);
        black_box(quiet);
    }
    let heard = life.hear(&inbound, "where did you go?");
    black_box(heard);
    ARMED.with(|armed| armed.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "turn or tick allocated");

    let mut life = SamanthaInner::new();
    let mut core = EmotionCore::at_rest();
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for _ in 0..40u32 {
        let note = life
            .nudge(
                &mut core,
                Perceived {
                    t_ms: 0,
                    words: "where did you go?",
                    ear: None,
                    sense: false,
                },
            )
            .unwrap();
        black_box((note, core.moment()));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "nudge allocated");
}

#[test]
fn tick_and_turn_do_not_allocate_with_a_configured_home() {
    let _hold = GATE.lock().expect("lock");
    let home = InnerHome {
        care: 0.11,
        curiosity: 0.73,
        help: 0.19,
        expect: 0.61,
    };
    let mut core = EmotionCore::at_rest();
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    let built = InnerHome::from_baseline(0.10, 0.73, 0.19, 0.61).expect("home");
    black_box(built);
    let mut life = PersonaInner::with_home_for("northwind", home).expect("id");
    for i in 0..40u32 {
        let turn = life.turn(Perceived {
            t_ms: i * TICK_MS,
            words: "I feel really sad",
            ear: Some(Heard {
                valence: -0.4,
                arousal: 0.3,
                confidence: 0.8,
            }),
            sense: false,
        });
        black_box(turn);
        let quiet = life.tick(TICK_MS, true);
        black_box(quiet);
    }
    let note = life
        .nudge(
            &mut core,
            Perceived {
                t_ms: 0,
                words: "where did you go?",
                ear: None,
                sense: false,
            },
        )
        .expect("nudge");
    black_box(note);
    let back = life.reset();
    black_box((back, life.persona_id()));
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "configured home allocated"
    );
    assert_eq!(life.persona_id(), "northwind");
    assert_eq!(life.care().to_bits(), home.care.to_bits());
}
