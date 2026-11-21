// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Single-producer, single-consumer ring.
//!
//! One thread calls [`Spsc::push_slice`]. One thread calls [`Spsc::pop_slice`]
//! and [`Spsc::apply_skip`]. Any thread may call [`Spsc::request_skip`]: that
//! publishes the current tail, and the consumer drops every sample up to it
//! the next time it looks. Samples pushed after `request_skip` returns stay
//! in the ring.
//!
//! Indices are monotonic `usize` counters. A full ring holds `capacity`
//! samples (the capacity is rounded up to a power of two). The empty and full
//! states are `tail - head == 0` and `tail - head == capacity`.
//!
//! The only unsafe in this crate is the slot access below. Slots are
//! `Copy`, so an overwrite does not drop a value the consumer still owns.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[repr(align(64))]
struct CacheLine<T> {
    value: T,
}

struct Shared<T> {
    buf: Box<[UnsafeCell<T>]>,
    cap: usize,
    mask: usize,
    /// Consumer-owned read cursor.
    head: CacheLine<AtomicUsize>,
    /// Producer-owned write cursor.
    tail: CacheLine<AtomicUsize>,
    /// Samples with index `< skip` are discarded by the consumer.
    skip: CacheLine<AtomicUsize>,
    discarded: AtomicU64,
}

// SAFETY: see the module docs. Producer and consumer never touch the same
// index. `head` is stored by the consumer, `tail` by the producer, `skip`
// by `request_skip`. A consumer read of a slot happens after an Acquire
// load of `tail`, which happens after the producer's Release store of that
// tail, which happens after the slot write. A skip jumps `head` forward
// without reading those slots.
unsafe impl<T: Copy + Send> Sync for Shared<T> {}

pub(crate) struct Spsc<T> {
    inner: Arc<Shared<T>>,
}

impl<T> Clone for Spsc<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Copy + Default + Send> Spsc<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        let cap = capacity.max(2).next_power_of_two();
        let buf = (0..cap).map(|_| UnsafeCell::new(T::default())).collect();
        Self {
            inner: Arc::new(Shared {
                buf,
                cap,
                mask: cap - 1,
                head: CacheLine {
                    value: AtomicUsize::new(0),
                },
                tail: CacheLine {
                    value: AtomicUsize::new(0),
                },
                skip: CacheLine {
                    value: AtomicUsize::new(0),
                },
                discarded: AtomicU64::new(0),
            }),
        }
    }

    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.inner.cap
    }

    pub(crate) fn tail(&self) -> usize {
        self.inner.tail.value.load(Ordering::Acquire)
    }

    pub(crate) fn head(&self) -> usize {
        self.inner.head.value.load(Ordering::Acquire)
    }

    pub(crate) fn skip(&self) -> usize {
        self.inner.skip.value.load(Ordering::Acquire)
    }

    pub(crate) fn discarded(&self) -> u64 {
        self.inner.discarded.load(Ordering::Relaxed)
    }

    /// Samples between head and tail, including ones a pending skip will drop.
    pub(crate) fn queued(&self) -> usize {
        self.tail().wrapping_sub(self.head())
    }

    /// Free slots. Producer-side estimate. The consumer only frees more.
    pub(crate) fn free(&self) -> usize {
        let tail = self.inner.tail.value.load(Ordering::Relaxed);
        let head = self.inner.head.value.load(Ordering::Acquire);
        self.inner.cap - tail.wrapping_sub(head)
    }

    pub(crate) fn push_slice(&self, data: &[T]) -> usize {
        if data.is_empty() {
            return 0;
        }
        let tail = self.inner.tail.value.load(Ordering::Relaxed);
        let head = self.inner.head.value.load(Ordering::Acquire);
        let free = self.inner.cap - tail.wrapping_sub(head);
        let n = data.len().min(free);
        for (i, item) in data.iter().take(n).enumerate() {
            let idx = tail.wrapping_add(i) & self.inner.mask;
            // SAFETY: this index is unpublished until the Release store below.
            // The consumer only reads indices in `head..tail`.
            unsafe { *self.inner.buf[idx].get() = *item }
        }
        self.inner
            .tail
            .value
            .store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    pub(crate) fn push(&self, value: T) -> bool {
        self.push_slice(&[value]) == 1
    }

    /// Publish "drop everything queued right now". Lock-free.
    pub(crate) fn request_skip(&self) -> usize {
        let tail = self.inner.tail.value.load(Ordering::Acquire);
        self.inner.skip.value.fetch_max(tail, Ordering::Release);
        tail
    }

    /// Consumer only. Moves `head` forward to the skip point when that point
    /// still lies in the queued range. Returns how many samples were dropped.
    pub(crate) fn apply_skip(&self) -> u64 {
        let skip = self.inner.skip.value.load(Ordering::Acquire);
        let head = self.inner.head.value.load(Ordering::Relaxed);
        let tail = self.inner.tail.value.load(Ordering::Acquire);
        let queued = tail.wrapping_sub(head);
        let ahead = skip.wrapping_sub(head);
        if ahead == 0 || ahead > queued {
            return 0;
        }
        self.inner
            .head
            .value
            .store(head.wrapping_add(ahead), Ordering::Release);
        self.inner
            .discarded
            .fetch_add(ahead as u64, Ordering::Relaxed);
        ahead as u64
    }

    pub(crate) fn pop_slice(&self, dst: &mut [T]) -> usize {
        if dst.is_empty() {
            return 0;
        }
        let head = self.inner.head.value.load(Ordering::Relaxed);
        let tail = self.inner.tail.value.load(Ordering::Acquire);
        let n = dst.len().min(tail.wrapping_sub(head));
        for (i, slot) in dst.iter_mut().take(n).enumerate() {
            let idx = head.wrapping_add(i) & self.inner.mask;
            // SAFETY: `head..tail` was published by the producer's Release
            // store, observed by the Acquire load above. The producer will
            // not reuse the slot until it sees this head move.
            *slot = unsafe { *self.inner.buf[idx].get() };
        }
        if n > 0 {
            self.inner
                .head
                .value
                .store(head.wrapping_add(n), Ordering::Release);
        }
        n
    }

    pub(crate) fn pop(&self) -> Option<T> {
        let mut slot = [T::default()];
        if self.pop_slice(&mut slot) == 1 {
            Some(slot[0])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    #[test]
    fn capacity_is_a_power_of_two_and_the_ring_wraps() {
        let ring = Spsc::<u32>::new(3);
        assert_eq!(ring.capacity(), 4);
        assert!(ring.push_slice(&[1, 2, 3, 4]) == 4);
        assert!(!ring.push(5));
        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
        assert!(ring.push(5));
        assert!(ring.push(6));
        assert!(!ring.push(7));
        let mut out = [0; 4];
        assert_eq!(ring.pop_slice(&mut out), 4);
        assert_eq!(out, [3, 4, 5, 6]);
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn skip_drops_what_was_queued_and_keeps_what_follows() {
        let ring = Spsc::<u32>::new(16);
        assert_eq!(ring.push_slice(&[1, 2, 3, 4]), 4);
        let at = ring.request_skip();
        assert_eq!(at, 4);
        assert_eq!(ring.push_slice(&[9, 8]), 2);
        let dropped = ring.apply_skip();
        assert_eq!(dropped, 4);
        assert_eq!(ring.discarded(), 4);
        assert_eq!(ring.pop(), Some(9));
        assert_eq!(ring.pop(), Some(8));
        assert_eq!(ring.pop(), None);
        assert_eq!(ring.apply_skip(), 0);
    }

    #[test]
    fn concurrent_spsc_preserves_order() {
        let ring = Spsc::<u32>::new(128);
        let producer = ring.clone();
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        let handle = thread::spawn(move || {
            for i in 0..50_000u32 {
                while !producer.push(i) {
                    thread::yield_now();
                }
            }
            flag.store(true, Ordering::Release);
        });
        let mut expect = 0u32;
        let mut empty_after_done = 0u32;
        while expect < 50_000 {
            match ring.pop() {
                Some(v) => {
                    assert_eq!(v, expect);
                    expect += 1;
                    empty_after_done = 0;
                }
                None => {
                    if done.load(Ordering::Acquire) {
                        empty_after_done += 1;
                        assert!(
                            empty_after_done < 100_000,
                            "ring stalled at {expect} after the producer finished"
                        );
                    }
                    thread::yield_now();
                }
            }
        }
        handle.join().unwrap();
    }

    #[test]
    fn skip_from_a_third_thread_still_delivers_a_later_sentinel() {
        let ring = Spsc::<u32>::new(256);
        let producer = ring.clone();
        let flusher = ring.clone();
        let consumer = ring.clone();
        let start = Arc::new(AtomicBool::new(false));
        let s1 = Arc::clone(&start);
        let s2 = Arc::clone(&start);
        let prod = thread::spawn(move || {
            while !s1.load(Ordering::Acquire) {
                thread::yield_now();
            }
            for i in 0..1_000u32 {
                while !producer.push(i) {
                    thread::yield_now();
                }
            }
            while !producer.push(u32::MAX) {
                thread::yield_now();
            }
        });
        let flush = thread::spawn(move || {
            while !s2.load(Ordering::Acquire) {
                thread::yield_now();
            }
            for _ in 0..100 {
                flusher.request_skip();
                thread::yield_now();
            }
        });
        start.store(true, Ordering::Release);
        // Keep popping so a full ring cannot block the producer while this
        // thread waits. `pop` does not apply a skip, so the sentinel stays
        // reachable. `request_skip` running beside it must not tear a slot.
        let mut saw = false;
        for _ in 0..10_000_000 {
            if consumer.pop() == Some(u32::MAX) {
                saw = true;
                break;
            }
            thread::yield_now();
        }
        assert!(saw, "sentinel was not delivered");
        prod.join().unwrap();
        flush.join().unwrap();
    }
}
