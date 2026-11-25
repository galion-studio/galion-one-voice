// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Counting allocator for the duplex soak. The CI run is five virtual
//! minutes. `GALION_SOAK_FULL=1` runs sixty.
//!
//! The tick gate follows `docs/horbm/foundations/S56-soak-timing.md`:
//! thread CPU time on Unix, wall time elsewhere, after warmup.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_soak::{minutes_from_env, run, TickClock};

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

fn soak_clock() -> TickClock {
    #[cfg(unix)]
    {
        TickClock::ThreadCpu(thread_cpu_ns)
    }
    #[cfg(not(unix))]
    {
        TickClock::Wall
    }
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
            // SAFETY: `addr` is a readable mapped byte from `/proc/self/maps`.
            // The read is volatile so the fault is not deleted. The pointer is not retained.
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
    #[cfg(unix)]
    {
        let started = thread_cpu_ns();
        let mut acc = 0u64;
        for i in 0..200_000u64 {
            acc = acc.wrapping_add(i);
        }
        std::hint::black_box(acc);
        let stopped = thread_cpu_ns();
        assert!(
            stopped > started,
            "thread cpu clock did not advance ({started} -> {stopped})"
        );
        assert!(
            clock_res_ns() > 0,
            "CLOCK_THREAD_CPUTIME_ID did not report a resolution"
        );
    }

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
    let clock = soak_clock();
    eprintln!("soak-clock name={} res_ns={}", clock.name(), clock_res_ns());
    let report = run(minutes, &allocs, clock);
    println!("{}", report.table());
    println!("{}", report.stages());
    ARMED.with(|armed| armed.set(false));
    assert_eq!(report.clock, clock.name());
    #[cfg(unix)]
    assert_eq!(report.clock, "thread-cpu");
    #[cfg(not(unix))]
    assert_eq!(report.clock, "wall");
    report.enforce().expect("soak gate");
}

#[cfg(unix)]
fn clock_res_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a live timespec. The call does not retain the pointer.
    // A failed resolution read is printed as 0. The clock read itself fails closed.
    let rc = unsafe { libc::clock_getres(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    if rc != 0 {
        return 0;
    }
    timespec_ns(ts.tv_sec, ts.tv_nsec).unwrap_or(0)
}

#[cfg(not(unix))]
fn clock_res_ns() -> u64 {
    0
}

#[cfg(unix)]
fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a live timespec. CLOCK_THREAD_CPUTIME_ID is the POSIX
    // per-thread CPU-time clock. The call does not retain the pointer.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(
        rc,
        0,
        "clock_gettime(CLOCK_THREAD_CPUTIME_ID): {}",
        std::io::Error::last_os_error()
    );
    timespec_ns(ts.tv_sec, ts.tv_nsec).unwrap_or_else(|| {
        panic!("negative timespec {}s {}ns", ts.tv_sec, ts.tv_nsec);
    })
}

#[cfg(unix)]
fn timespec_ns(sec: libc::time_t, nsec: libc::c_long) -> Option<u64> {
    let sec = u64::try_from(sec).ok()?;
    let nsec = u64::try_from(nsec).ok()?;
    Some(sec.saturating_mul(1_000_000_000).saturating_add(nsec))
}
