// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Counting allocator: `RikaReplayBackend::step_live` (Heartbeat::step + mock) allocates nothing.
use galion_emotion::band::consent_bits;
use galion_emotion::Purpose;
use galion_heartbeat::presence::RikaReplayBackend;
use galion_heartbeat::FRAME_SAMPLES;
use galion_live_demo::{heartbeat_config, synth_you};
use galion_presence::backend::FrameIn;
use galion_presence::spec::YouState;
use galion_presence::wav;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static A: Counting = Counting;

#[test]
fn step_live_allocates_nothing_with_rika_opener_on() {
    let you = synth_you();
    let mut b = RikaReplayBackend::with_config(heartbeat_config(true));
    let bits = consent_bits::SENSE | consent_bits::ADAPT;
    let mut lean = 0;
    for (i, fr) in you.chunks(FRAME_SAMPLES).enumerate() {
        let db = wav::rms_dbfs(fr);
        let st = if db >= -35.0 { YouState::Speech } else if db >= -55.0 { YouState::Breath } else { YouState::Silence };
        let f = FrameIn { index: i as u32, you: st, affect_in: None, adapt: true, late: i % 50 == 49, me_planned: false };
        ARMED.with(|a| a.set(true));
        let (_, ho) = b.step_live(&f, fr, bits, Purpose::PersonalAssistant);
        ARMED.with(|a| a.set(false));
        lean += ho.lean_in_audible() as u32;
    }
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "step_live allocated");
    assert!(lean > 0);
    assert!(b.heartbeat().paused_frames() >= 3, "late frames are pauses");
}
