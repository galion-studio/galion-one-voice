// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! A6: `encode_frame` / `decode_frame` (and the latent variants) perform zero heap allocations
//! after construction, both kernel-free and with real A5 kernels. Counting allocator, own binary.

mod common;
use common::*;
use galion_mimi::{MimiCodec, MimiConfig};
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

fn exercise(m: &mut MimiCodec, n_cb: usize, d: usize) -> usize {
    let pcm = test_pcm(5 * 1920);
    let mut codes = vec![0u16; n_cb];
    let mut out = vec![0.0f32; 1920];
    let mut lat = vec![0.0f32; d];
    let before = allocs();
    for fr in pcm.chunks_exact(1920) {
        m.encode_frame(fr, &mut codes).unwrap();
        m.decode_frame(&codes, &mut out).unwrap();
        m.encode_latent_frame(fr, &mut lat).unwrap();
        m.decode_latent_frame(&lat, &mut out).unwrap();
    }
    m.reset();
    allocs() - before
}

#[test]
fn frame_loop_alloc_free_with_kernels() {
    let cfg = tiny_cfg();
    let mut m = MimiCodec::with_weight_source(cfg.clone(), &SyntheticWeights).unwrap();
    assert_eq!(exercise(&mut m, cfg.n_codebooks, cfg.latent_dim), 0);
}

#[test]
fn frame_loop_alloc_free_without_weights() {
    let cfg = MimiConfig::default();
    let mut m = MimiCodec::new(cfg.clone()).unwrap();
    assert_eq!(exercise(&mut m, cfg.n_codebooks, cfg.latent_dim), 0);
}

#[test]
fn counter_detects_allocations() {
    let before = allocs();
    let v: Vec<u8> = Vec::with_capacity(16);
    drop(v);
    assert!(allocs() > before);
}
