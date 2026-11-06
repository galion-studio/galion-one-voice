// Galion Voice — Perpetual Open MIT License
//! The persona-mood hot path must not touch the heap.
//! The duplex clock, consented store record, FiLM apply, phone lane, and
//! spoken classification are in the same window. The budget stays zero.
use galion_emotion::store::spoken::{self, Heard};
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
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::SeqCst);
        System.realloc(p, l, new_size)
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
    let uttered = ["Da, može.", "no thanks", "Galion, prestani!", "hmm"];
    let mut gate = DefaultConsentGate::default();
    gate.grant(0, "hr", Purpose::PersonalAssistant);
    let mut store = AffectStore::new(8)
        .grant(&gate, Purpose::PersonalAssistant, consent_bits::SENSE | consent_bits::ADAPT)
        .unwrap();
    let mut clock = EmotionClock::new();
    let film = StubFilm::new(4, 0.2);
    let mut params = FilmParams::identity(4);
    let mut hidden = [0.1f32; 4];
    let live = ALLOCS.load(Ordering::SeqCst);
    drop(std::hint::black_box(Box::new(0u8)));
    assert!(ALLOCS.load(Ordering::SeqCst) > live, "counting allocator is live");
    let before = ALLOCS.load(Ordering::SeqCst);
    let mut acc = 0.0f32;
    for i in 0..10_000u64 {
        let t = text_affect(texts[(i % 4) as usize]);
        let o = UserObservation::fuse(Some(&voice), false, Some(t));
        s.observe(&o, i * 80);
        j.observe(&o, i * 80);
        let cs = s.conditioning(i * 80 + 40);
        let cj = j.conditioning(i * 80 + 40);
        let felt_s = s.feeling(i * 80 + 40);
        let felt_j = j.feeling(i * 80 + 40);
        acc += cs.piper().length_scale + cj.piper().pitch_shift_st + cs.to_array()[3];
        acc += felt_s.e + felt_j.prosody.energy + s.timbre().match_strength + j.timbre().match_strength;
        let frame = o.as_frame();
        store.record(&frame);
        let color = clock.on_frame(store.adapt_target().as_ref());
        let lane = StubExpressor.phone_lane(
            store.adapt_target().as_ref(),
            consent_bits::SENSE | consent_bits::ADAPT,
            Purpose::PersonalAssistant,
            &gate,
        );
        film.condition(Some(&color.target), &mut params);
        apply_film(&mut hidden, &params);
        acc += if check_counterbalance(&frame, &color.target).is_ok() { 0.01 } else { 0.02 };
        acc += color.e + lane.e + hidden[0];
        acc += match spoken::classify(uttered[(i % 4) as usize]) {
            Heard::Yes => 0.1,
            Heard::No => 0.2,
            Heard::Withdraw => 0.3,
            Heard::Unclear => 0.4,
        };
    }
    let after = ALLOCS.load(Ordering::SeqCst);
    assert!(acc.is_finite());
    assert_eq!(after - before, 0, "hot path allocated {} times", after - before);
}
