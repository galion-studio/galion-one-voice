// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Zero allocation in the frame loop: MockBackend::step must never touch the heap.
use galion_presence::backend::{AffectIn, FrameIn, MockBackend, MockConfig, ReplayBackend};
use galion_presence::spec::YouState;
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
fn mock_step_does_not_allocate() {
    let mut m = MockBackend::new(MockConfig::default());
    let frames: [FrameIn; 4] = [
        FrameIn { index: 0, you: YouState::Silence, affect_in: Some(AffectIn { valence: 0.0, arousal: 0.3 }), adapt: true, late: false, me_planned: true },
        FrameIn { index: 1, you: YouState::Speech, affect_in: Some(AffectIn { valence: -0.6, arousal: 0.9 }), adapt: true, late: false, me_planned: true },
        FrameIn { index: 2, you: YouState::Breath, affect_in: None, adapt: false, late: true, me_planned: false },
        FrameIn { index: 3, you: YouState::Silence, affect_in: None, adapt: false, late: false, me_planned: false },
    ];
    let before = ALLOCS.load(Ordering::SeqCst);
    let mut voiced = 0u32;
    for i in 0..10_000u32 {
        let o = m.step(&frames[(i % 4) as usize]);
        voiced += o.me_voiced as u32;
    }
    let after = ALLOCS.load(Ordering::SeqCst);
    assert_eq!(after - before, 0, "allocations inside the frame loop");
    assert!(voiced > 0);
}
