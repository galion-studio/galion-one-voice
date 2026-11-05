// Galion Voice — Perpetual Open MIT License
//! The persona-mood hot path must not touch the heap.
use galion_emotion::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}
#[global_allocator]
static A: Counting = Counting;

#[test]
fn observe_condition_prosody_text_are_allocation_free() {
    let mut s = PersonaMood::new(Persona::Samantha);
    let mut j = PersonaMood::new(Persona::Jarvis);
    let voice = EmotionFrame { seq: 1, t_ms: 0, dur_ms: 80, valence: 0.0, arousal: 0.7, dominance: 0.5, confidence: 0.3 };
    let texts = ["My dog died and I'm so sad.", "This is amazing!!", "not happy", "set a timer"];
    let before = ALLOCS.load(Ordering::SeqCst);
    let mut acc = 0.0f32;
    for i in 0..10_000u64 {
        let t = text_affect(texts[(i % 4) as usize]);
        let o = UserObservation::fuse(Some(&voice), false, Some(t));
        s.observe(&o, i * 80);
        j.observe(&o, i * 80);
        let cs = s.conditioning(i * 80 + 40);
        let cj = j.conditioning(i * 80 + 40);
        acc += cs.piper().length_scale + cj.piper().pitch_shift_st + cs.to_array()[3];
    }
    let after = ALLOCS.load(Ordering::SeqCst);
    assert!(acc.is_finite());
    assert_eq!(after - before, 0, "hot path allocated {} times", after - before);
}
