// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `set_moment` and `process` allocate nothing on the 80 ms tick.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use galion_source_filter::{
    frame_samples, Breath, BreathPhase, MomentState, MouthControls, SourceFilter, VoiceBaseline,
    FRAME_ALLOC_BUDGET,
};

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
fn the_frame_path_stays_inside_the_alloc_budget() {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    assert_eq!(FRAME_ALLOC_BUDGET, 0);
    drop(probe);

    let rate = 24_000u32;
    let width = frame_samples(rate);
    let mut filter = SourceFilter::samantha(rate);
    let voice = VoiceBaseline::standin();
    let mut moment = MomentState::at(voice.pos);
    let exhale = Breath {
        phase: BreathPhase::Exhale,
        volume: 0.4,
        flow: -0.6,
    };
    filter.set_moment(&voice, &moment, exhale, 0.3);
    let mut buf = vec![0.15f32; width];
    filter.process(&mut buf);

    let held = [
        MouthControls {
            tension: 0.4,
            openness: 0.2,
            nasal: 0.5,
            breath_noise: 0.0,
            ..MouthControls::neutral()
        },
        MouthControls::neutral(),
    ];
    let mut pair = vec![0.1f32; width * 2];

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 0..200u32 {
        moment.pos[1] = 0.2 + (i % 5) as f32 * 0.1;
        moment.t_ms = i * 80;
        moment.tick = i;
        let breath = if i % 2 == 0 { exhale } else { Breath::rest() };
        filter.set_moment(&voice, &moment, breath, 0.25);
        filter.process(&mut buf);
        if i % 7 == 0 {
            filter.process_frames(&mut pair, &held);
        }
        black_box((buf[0], pair[0], filter.controls()));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        FRAME_ALLOC_BUDGET,
        "the 80 ms mouth frame allocated"
    );
}
