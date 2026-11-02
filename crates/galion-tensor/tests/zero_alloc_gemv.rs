// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! A4: GEMV hot paths (f32, transposed f32, int8 W8A8) perform zero heap allocations per call
//! once buffers exist (scalar exec; the rayon path may allocate inside the pool).

use galion_tensor::gemv::{gemv_f32_into, gemv_t_f32_into};
use galion_tensor::gemv_i8::{gemv_i8_f32x_into, QuantRowsI8};
use galion_tensor::{features, Exec};
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
fn gemv_paths_are_alloc_free() {
    let (m, k) = (96, 160);
    let a: Vec<f32> = (0..m * k).map(|i| ((i % 17) as f32 - 8.0) * 0.01).collect();
    let x: Vec<f32> = (0..k).map(|i| ((i % 5) as f32 - 2.0) * 0.1).collect();
    let xt: Vec<f32> = (0..m).map(|i| ((i % 7) as f32 - 3.0) * 0.1).collect();
    let b = vec![0.25f32; m];
    let w = QuantRowsI8::quantize(&a, m, k).unwrap();
    let mut y = vec![0f32; m];
    let mut yt = vec![0f32; k];
    let mut scratch = vec![0i8; k];
    let level = features::selected(); // warm the OnceLock (env read) before counting
    let before = allocs();
    for _ in 0..50 {
        gemv_f32_into(&a, m, k, &x, Some(&b), &mut y, Exec::Scalar).unwrap();
        gemv_t_f32_into(level, &xt, &a, m, k, None, &mut yt, Exec::Scalar).unwrap();
        gemv_i8_f32x_into(&w, &x, &mut scratch, Some(&b), &mut y, Exec::Scalar).unwrap();
    }
    assert_eq!(allocs() - before, 0, "GEMV hot path allocated (level {level:?})");
}
