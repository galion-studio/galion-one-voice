// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! A5: hot-path `step` calls must perform zero heap allocations after init.
//! Uses a counting global allocator scoped to this test binary (thread-local counter).

use galion_tensor::{CausalConv1dStream, ConvTranspose1dStream, KvRing, StreamingAttention, StridedConv1dStream, Tensor};
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
fn conv_stream_step_is_alloc_free() {
    let w = Tensor::from_vec((0..4 * 4 * 5).map(|i| i as f32 * 0.01).collect(), [4, 4, 5]).unwrap();
    let mut s = CausalConv1dStream::new(&w, None, 2, 1, 32).unwrap();
    let x = vec![0.5f32; 4 * 32];
    let mut y = vec![0f32; 4 * 32];
    let before = allocs();
    for _ in 0..100 { s.step(&x, 32, &mut y).unwrap(); s.step(&x[..4 * 7], 7, &mut y[..4 * 7]).unwrap(); }
    s.reset();
    assert_eq!(allocs() - before, 0);
}

#[test]
fn kv_and_attention_step_are_alloc_free() {
    let mut r = KvRing::new(8, 2, 4).unwrap();
    let mut a = StreamingAttention::new(4, 2, 8, 12, 10_000.0, 0).unwrap();
    let (q, k, v) = (vec![0.1f32; 32], vec![0.2f32; 16], vec![0.3f32; 16]);
    let mut o = vec![0f32; 32];
    let before = allocs();
    for _ in 0..50 { r.push(&k[..8], &v[..8]).unwrap(); a.step(&q, &k, &v, &mut o).unwrap(); } // wraps both rings
    a.reset();
    assert_eq!(allocs() - before, 0);
}

#[test]
fn strided_and_transposed_conv_step_are_alloc_free() {
    // Mimi-like: encoder downsample k=8 s=4 on 1920-sample frames; decoder upsample k=16 s=8.
    let w = Tensor::from_vec((0..2 * 2 * 8).map(|i| i as f32 * 0.01).collect(), [2, 2, 8]).unwrap();
    let mut s = StridedConv1dStream::new(&w, None, 4, 1, 1, 1920).unwrap();
    let x = vec![0.5f32; 2 * 1920];
    let mut y = vec![0f32; 2 * s.max_outputs_per_step()];
    let wt = Tensor::from_vec((0..2 * 3 * 16).map(|i| i as f32 * 0.01).collect(), [2, 3, 16]).unwrap();
    let mut t = ConvTranspose1dStream::new(&wt, Some(&Tensor::zeros([3])), 8, 1, 1, 4).unwrap();
    let xt = vec![0.25f32; 2 * 4];
    let mut yt = vec![0f32; 3 * 4 * 8];
    let before = allocs();
    for _ in 0..20 {
        s.step(&x, 1920, &mut y).unwrap();
        s.step(&x[..2 * 7], 7, &mut y).unwrap();
        t.step(&xt, 4, &mut yt).unwrap();
        t.step(&xt[..2], 1, &mut yt[..3 * 8]).unwrap();
    }
    s.reset();
    t.reset();
    assert_eq!(allocs() - before, 0);
}

#[test]
fn counter_detects_allocations() {
    let before = allocs();
    let v: Vec<f32> = std::hint::black_box(vec![1.0; 16]);
    assert!(allocs() > before && v.len() == 16, "negative control: allocator counter must see a Vec");
}
