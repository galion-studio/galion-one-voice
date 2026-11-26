// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Counting allocator for the duplex soak. The CI run is five virtual
//! minutes. `GALION_SOAK_FULL=1` runs sixty.
//!
//! `ALLOCS` counts allocation events on this thread while armed. That count
//! is the steady-path gate. `LIVE_BYTES` is requested bytes still outstanding
//! on every thread. The growth gate reads `LIVE_BYTES`, not `VmRSS`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_soak::{minutes_from_env, run};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
/// Requested bytes still live. Alloc adds, dealloc subtracts, realloc applies
/// the size delta. Not jemalloc.
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

fn live_add(bytes: usize) {
    LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed);
}

fn live_sub(bytes: usize) {
    LIVE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
}

/// One update, so a sample cannot land between the free and the new alloc.
fn live_resize(old: usize, new: usize) {
    if new >= old {
        live_add(new - old);
    } else {
        live_sub(old - new);
    }
}

fn live_heap_bytes() -> u64 {
    LIVE_BYTES.load(Ordering::Relaxed) as u64
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            if ARMED.with(|armed| armed.get()) {
                ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            live_add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        live_sub(layout.size());
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = System.realloc(ptr, layout, new_size);
        if !next.is_null() && ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // A null pointer with `new_size == 0` released the old block.
        // Any other null leaves that block in place.
        if !next.is_null() || new_size == 0 {
            let kept = if next.is_null() { 0 } else { new_size };
            live_resize(layout.size(), kept);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed) as u64
}

/// Fault every current file mapping.
///
/// The driver calls this again immediately before the warm sample and the
/// end sample. Those samples print `VmRSS` and smaps `Rss`. A file page
/// reclaimed between the reads is faulted back for both, so it is not
/// resident-set growth. The gate is live heap and does not count these pages.
#[cfg(target_os = "linux")]
fn prefault_file_mappings() {
    use std::cell::RefCell;
    use std::io::Read;
    thread_local! {
        static MAPS: RefCell<String> = const { RefCell::new(String::new()) };
    }
    let mut failed = false;
    MAPS.with(|slot| {
        let mut text = slot.borrow_mut();
        text.clear();
        let Ok(mut file) = std::fs::File::open("/proc/self/maps") else {
            failed = true;
            return;
        };
        if file.read_to_string(&mut text).is_err() {
            failed = true;
        }
    });
    if failed {
        return;
    }
    MAPS.with(|slot| {
        let text = slot.borrow();
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
    });
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
    let report = run(minutes, &allocs, &live_heap_bytes, &prefault_file_mappings);
    println!("{}", report.table());
    println!("{}", report.stages());
    ARMED.with(|armed| armed.set(false));
    report.enforce().expect("soak gate");
}
