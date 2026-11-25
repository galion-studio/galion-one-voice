// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Counting allocator for the duplex soak. The CI run is five virtual
//! minutes. `GALION_SOAK_FULL=1` runs sixty.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_soak::{minutes_from_env, run};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() && ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = System.realloc(ptr, layout, new_size);
        if !next.is_null() && ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed) as u64
}

/// The style index is linked into this binary. Fault its pages before the
/// soak samples resident memory, so a later call does not look like a leak.
#[cfg(target_os = "linux")]
fn prefault_file_mappings() {
    let Ok(text) = std::fs::read_to_string("/proc/self/maps") else {
        return;
    };
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(range) = parts.next() else {
            continue;
        };
        let Some(perms) = parts.next() else {
            continue;
        };
        if !perms.contains('r') {
            continue;
        }
        let path = line.splitn(6, ' ').nth(5).unwrap_or("").trim();
        if path.is_empty() || path.starts_with('[') {
            continue;
        }
        let Some((start_hex, end_hex)) = range.split_once('-') else {
            continue;
        };
        let Ok(start) = usize::from_str_radix(start_hex, 16) else {
            continue;
        };
        let Ok(end) = usize::from_str_radix(end_hex, 16) else {
            continue;
        };
        let mut addr = start;
        while addr < end {
            unsafe {
                std::ptr::read_volatile(addr as *const u8);
            }
            addr = addr.saturating_add(4096);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn prefault_file_mappings() {}

#[test]
fn samantha_duplex_long_call() {
    prefault_file_mappings();
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8; 8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::Relaxed) > 0,
        "the counter must see a Vec"
    );
    drop(probe);
    ALLOCS.store(0, Ordering::Relaxed);
    ARMED.with(|armed| armed.set(true));

    let minutes = minutes_from_env(std::env::var("GALION_SOAK_FULL").ok().as_deref());
    let report = run(minutes, &allocs);
    println!("{}", report.table());
    println!("{}", report.stages());
    ARMED.with(|armed| armed.set(false));
    report.enforce().expect("soak gate");
}
