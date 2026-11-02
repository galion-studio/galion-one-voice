// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-02: `MimiPassthrough::step_frame` / `StudentBackend::step_frame` do not allocate.
use galion_student_backend::mimi_lane::tone_frame;
use galion_student_backend::{KernelMode, MimiLaneConfig, MimiPassthrough, Monitor, StudentBackend, FRAME_SAMPLES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;
thread_local! { static ALLOCS: Cell<usize> = const { Cell::new(0) }; }
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { let _ = ALLOCS.try_with(|c| c.set(c.get() + 1)); System.alloc(l) }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 { let _ = ALLOCS.try_with(|c| c.set(c.get() + 1)); System.realloc(p, l, n) }
}
#[global_allocator]
static GLOBAL: Counting = Counting;
fn allocs() -> usize { ALLOCS.with(|c| c.get()) }

#[test]
fn step_frame_alloc_free() {
    let frames: Vec<Vec<f32>> = (0..6).map(|f| tone_frame(f, 0.5)).collect();
    let mut out = vec![0.0f32; FRAME_SAMPLES];
    for cfg in [
        MimiLaneConfig::demo(),
        MimiLaneConfig::demo().with_monitor(Monitor::DryBypass),
        MimiLaneConfig { kernels: KernelMode::NoKernels, ..MimiLaneConfig::demo() },
    ] {
        let mut b = StudentBackend::with_mimi(MimiPassthrough::new(cfg).unwrap());
        let before = allocs();
        for f in &frames {
            b.step_frame(f, &mut out).unwrap();
        }
        b.step_frame(&f32_short(), &mut out).unwrap(); // paused frame
        b.mimi_mut().unwrap().reset();
        assert_eq!(allocs() - before, 0);
    }
}

fn f32_short() -> [f32; 4] { [0.0; 4] }
