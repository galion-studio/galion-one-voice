// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
#![allow(unsafe_code)]

//! Thread CPU time. Wall time is [`std::time::Instant`] at the call site.
//! Linux uses `CLOCK_THREAD_CPUTIME_ID`. Windows uses `GetThreadTimes`
//! (kernel + user). This module is the only `unsafe` in the crate.

/// Nanoseconds of CPU time this thread has consumed so far.
pub fn thread_cpu_ns() -> u64 {
    platform::now_ns()
}

/// Elapsed thread CPU time since `start`, or 0 if the clock went backwards.
pub fn since_ns(start: u64) -> u64 {
    thread_cpu_ns().saturating_sub(start)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    pub fn now_ns() -> u64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `ts` is a live timespec. CLOCK_THREAD_CPUTIME_ID is a valid clock id.
        let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        if rc != 0 {
            return 0;
        }
        (ts.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(ts.tv_nsec as u64)
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
mod platform {
    pub fn now_ns() -> u64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `ts` is a live timespec. CLOCK_THREAD_CPUTIME_ID is a valid clock id on POSIX.
        let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        if rc != 0 {
            return 0;
        }
        (ts.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(ts.tv_nsec as u64)
    }
}

#[cfg(windows)]
mod platform {
    #[repr(C)]
    struct FileTime {
        lo: u32,
        hi: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(
            thread: isize,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    fn ticks(t: FileTime) -> u64 {
        ((t.hi as u64) << 32) | t.lo as u64
    }

    pub fn now_ns() -> u64 {
        // SAFETY: the pointers are live FileTime slots. GetCurrentThread returns a
        // pseudo-handle that GetThreadTimes accepts and that the caller must not close.
        unsafe {
            let thread = GetCurrentThread();
            let mut created = FileTime { lo: 0, hi: 0 };
            let mut exited = FileTime { lo: 0, hi: 0 };
            let mut kernel = FileTime { lo: 0, hi: 0 };
            let mut user = FileTime { lo: 0, hi: 0 };
            if GetThreadTimes(thread, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
                return 0;
            }
            // FILETIME is 100-nanosecond units.
            ticks(kernel)
                .saturating_add(ticks(user))
                .saturating_mul(100)
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    pub fn now_ns() -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::thread_cpu_ns;

    #[test]
    fn cpu_clock_moves_when_the_thread_works() {
        let a = thread_cpu_ns();
        let mut x = 0u64;
        for i in 0..2_000_000u64 {
            x = x.wrapping_add(i.rotate_left((i & 15) as u32));
        }
        std::hint::black_box(x);
        let b = thread_cpu_ns();
        assert!(b > a, "thread cpu clock did not advance ({a} -> {b})");
    }
}
