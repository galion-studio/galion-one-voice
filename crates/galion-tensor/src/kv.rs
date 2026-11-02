// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Preallocated KV ring cache (sliding window) for streaming attention.
//!
//! Layout per slot: `[n_kv_heads, head_dim]` for K and V each; `capacity` slots allocated once in
//! [`KvRing::new`]. [`KvRing::push`] overwrites the oldest slot when full — **zero heap
//! allocation per frame**. Logical index `0` is always the oldest retained token, so iteration is
//! chronological (this ordering is what makes streaming attention bit-exact with the reference).

use crate::{Result, TensorError};

#[derive(Debug, Clone)]
pub struct KvRing {
    capacity: usize,
    n_kv_heads: usize,
    head_dim: usize,
    k: Vec<f32>,
    v: Vec<f32>,
    /// Physical slot of the oldest retained token.
    start: usize,
    len: usize,
    /// Total tokens ever pushed (absolute position of the next token).
    next_pos: usize,
}

impl KvRing {
    pub fn new(capacity: usize, n_kv_heads: usize, head_dim: usize) -> Result<Self> {
        if capacity == 0 || n_kv_heads == 0 || head_dim == 0 {
            return Err(TensorError::InvalidShape("kv ring: capacity, n_kv_heads, head_dim must be > 0".into()));
        }
        let n = capacity * n_kv_heads * head_dim;
        Ok(Self { capacity, n_kv_heads, head_dim, k: vec![0.0; n], v: vec![0.0; n], start: 0, len: 0, next_pos: 0 })
    }

    pub fn capacity(&self) -> usize { self.capacity }
    pub fn n_kv_heads(&self) -> usize { self.n_kv_heads }
    pub fn head_dim(&self) -> usize { self.head_dim }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.capacity }
    /// Absolute position the next pushed token will get.
    pub fn next_pos(&self) -> usize { self.next_pos }
    /// Absolute position of logical index 0 (oldest retained token).
    pub fn first_pos(&self) -> usize { self.next_pos - self.len }
    fn slot_len(&self) -> usize { self.n_kv_heads * self.head_dim }

    /// Drop all tokens (keeps the allocation).
    pub fn clear(&mut self) {
        self.start = 0;
        self.len = 0;
        self.next_pos = 0;
    }

    /// Append one token. `k`, `v`: `[n_kv_heads, head_dim]`. Evicts the oldest if full. No allocation.
    pub fn push(&mut self, k: &[f32], v: &[f32]) -> Result<()> {
        let sl = self.slot_len();
        if k.len() != sl || v.len() != sl {
            return Err(TensorError::ShapeMismatch { op: "kv push", lhs: vec![self.n_kv_heads, self.head_dim], rhs: vec![k.len(), v.len()] });
        }
        let slot = if self.len < self.capacity {
            let s = (self.start + self.len) % self.capacity;
            self.len += 1;
            s
        } else {
            let s = self.start;
            self.start = (self.start + 1) % self.capacity;
            s
        };
        self.k[slot * sl..(slot + 1) * sl].copy_from_slice(k);
        self.v[slot * sl..(slot + 1) * sl].copy_from_slice(v);
        self.next_pos += 1;
        Ok(())
    }

    #[inline]
    fn phys(&self, i: usize) -> usize {
        debug_assert!(i < self.len);
        (self.start + i) % self.capacity
    }

    /// Key of logical token `i` (0 = oldest), head `h`: `[head_dim]`.
    #[inline]
    pub fn key(&self, i: usize, h: usize) -> &[f32] {
        let o = (self.phys(i) * self.n_kv_heads + h) * self.head_dim;
        &self.k[o..o + self.head_dim]
    }

    /// Value of logical token `i` (0 = oldest), head `h`: `[head_dim]`.
    #[inline]
    pub fn value(&self, i: usize, h: usize) -> &[f32] {
        let o = (self.phys(i) * self.n_kv_heads + h) * self.head_dim;
        &self.v[o..o + self.head_dim]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_wraps_chronologically() {
        let mut r = KvRing::new(3, 1, 2).unwrap();
        assert!(r.is_empty());
        for t in 0..5 {
            let f = t as f32;
            r.push(&[f, f + 0.5], &[-f, -f - 0.5]).unwrap();
        }
        assert!(r.is_full());
        assert_eq!((r.len(), r.first_pos(), r.next_pos()), (3, 2, 5));
        assert_eq!(r.key(0, 0), &[2.0, 2.5]);
        assert_eq!(r.key(2, 0), &[4.0, 4.5]);
        assert_eq!(r.value(1, 0), &[-3.0, -3.5]);
        r.clear();
        assert_eq!((r.len(), r.next_pos()), (0, 0));
    }

    #[test]
    fn multi_head_layout_and_errors() {
        let mut r = KvRing::new(2, 2, 1).unwrap();
        r.push(&[1., 2.], &[3., 4.]).unwrap();
        assert_eq!((r.key(0, 1), r.value(0, 0)), (&[2.0][..], &[3.0][..]));
        assert!(r.push(&[1.], &[1., 2.]).is_err());
        assert!(KvRing::new(0, 1, 1).is_err());
    }
}
