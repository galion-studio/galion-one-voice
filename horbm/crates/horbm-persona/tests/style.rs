// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Sentence budget. The cap borrows a prefix of the spoken reply.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_persona::{Persona, UNKNOWN_LINE};

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

const VALID: &str = r#"
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

fn persona_with_id(id: &str) -> Persona {
    let text = VALID.replace("id = \"sample\"", &format!("id = \"{id}\""));
    Persona::parse(&text).unwrap_or_else(|err| panic!("parse {id}: {err}"))
}

fn northwind() -> Persona {
    let persona = persona_with_id("northwind");
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.style().max_sentences, 2);
    persona
}

#[test]
fn two_sentence_budget_keeps_two() {
    let style = northwind().style();
    let text = "The valve is open. The pump is running. The tank is full.";
    let kept = style.cap_sentences(text);
    assert_eq!(kept, "The valve is open. The pump is running. ");
    assert!(std::ptr::addr_eq(kept.as_ptr(), text.as_ptr()));
    assert!(text.starts_with(kept));

    let exact = "The valve is open. The pump is running.";
    assert_eq!(style.cap_sentences(exact), exact);

    let mixed = "Hold still. Look out! Is it clear? Go now.";
    assert_eq!(style.cap_sentences(mixed), "Hold still. Look out! ");

    let quoted = "She said \"Stop.\" He left. A third follows.";
    assert_eq!(style.cap_sentences(quoted), "She said \"Stop.\" He left. ");

    let bracket = "See the note (page 2.) He agreed. A third follows.";
    assert_eq!(
        style.cap_sentences(bracket),
        "See the note (page 2.) He agreed. "
    );

    let lines = "The valve is open.\nThe pump is running.\nThe tank is full.\n";
    assert_eq!(
        style.cap_sentences(lines),
        "The valve is open.\nThe pump is running.\n"
    );

    let single = "She said 'Stop.' He left. A third follows.";
    assert_eq!(style.cap_sentences(single), "She said 'Stop.' He left. ");
    assert_eq!(style.cap_sentences("Done.] Next. Third."), "Done.] Next. ");
    assert_eq!(style.cap_sentences("Done.} Next. Third."), "Done.} Next. ");

    let glued = "Go.Now stays one sentence until a real stop. Second. Third.";
    assert_eq!(
        style.cap_sentences(glued),
        "Go.Now stays one sentence until a real stop. Second. "
    );

    let mut zero = style;
    zero.max_sentences = 0;
    let kept = zero.cap_sentences(text);
    assert_eq!(kept, "The valve is open. ");
    assert!(!kept.is_empty());
}

#[test]
fn decimal_and_initial_do_not_split() {
    let style = northwind().style();
    let text = "The level is 3.5 today. J. R. confirmed the reading. The valve stays shut.";
    assert_eq!(
        style.cap_sentences(text),
        "The level is 3.5 today. J. R. confirmed the reading. "
    );
    assert!(std::ptr::addr_eq(
        style.cap_sentences(text).as_ptr(),
        text.as_ptr()
    ));

    let mut one = style;
    one.max_sentences = 1;
    assert_eq!(
        one.cap_sentences("The level is 3.5. Then more."),
        "The level is 3.5. "
    );
    assert_eq!(
        one.cap_sentences("J. R. R. wrote the note. Then he stopped."),
        "J. R. R. wrote the note. "
    );
    assert_eq!(one.cap_sentences(text), "The level is 3.5 today. ");
}

#[test]
fn no_stop_is_one_sentence() {
    let style = northwind().style();
    let text = "The pump is running and the valve is open";
    assert_eq!(style.cap_sentences(text), text);
    assert!(std::ptr::addr_eq(
        style.cap_sentences(text).as_ptr(),
        text.as_ptr()
    ));

    let mut one = style;
    one.max_sentences = 1;
    assert_eq!(one.cap_sentences(text), text);
    assert_eq!(one.cap_sentences(""), "");
    assert_eq!(style.cap_sentences(""), "");
    assert_eq!(one.cap_sentences("Open. Then wait"), "Open. ");
}

#[test]
fn unknown_line_passes_unchanged() {
    let style = northwind().style();
    let kept = style.cap_sentences(UNKNOWN_LINE);
    assert_eq!(kept, UNKNOWN_LINE);
    assert!(std::ptr::addr_eq(kept.as_ptr(), UNKNOWN_LINE.as_ptr()));

    let mut one = style;
    one.max_sentences = 1;
    let kept = one.cap_sentences(UNKNOWN_LINE);
    assert_eq!(kept, UNKNOWN_LINE);
    assert!(std::ptr::addr_eq(kept.as_ptr(), UNKNOWN_LINE.as_ptr()));
}

#[test]
#[allow(clippy::vec_init_then_push)] // the counter must see a push, not an empty Vec
fn cap_does_not_allocate() {
    ARMED.with(|armed| armed.set(true));
    let mut probe = Vec::<u8>::new();
    probe.push(1);
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    drop(probe);

    let style = northwind().style();
    let reply = "The valve is open. The level is 3.5. J. R. confirmed it. Next one.";
    let bare = "no stop in this reply";

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for _ in 0..1000 {
        black_box(style.cap_sentences(black_box(reply)));
        black_box(style.cap_sentences(black_box(UNKNOWN_LINE)));
        black_box(style.cap_sentences(black_box(bare)));
        black_box(style.cap_sentences(black_box("")));
        black_box(style.cap_sentences(black_box("She said \"Stop.\" He left. More.")));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "cap_sentences allocated");
}
