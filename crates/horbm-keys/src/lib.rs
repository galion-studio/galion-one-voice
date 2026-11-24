// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mapped int8 key arena for HORBM phase 3.
//!
//! The file is a structure of arrays. Keys are a contiguous int8 matrix,
//! 64-byte aligned in the file. Clause ids, a consent bitset, and source ids
//! sit beside the keys and are not touched by a denied-row skip.
//!
//! Unix opens the file with [`galion_tensor::MappedBytes`] (the same `mmap`
//! and [`Prefault`] policy as `.gq`). Windows and every other OS take that
//! type's heap read. `Prefault::Populate` or `Prefault::Touch` runs inside
//! `open`, before the header is accepted.
//!
//! The dot is [`galion_tensor::simd::dot_i8_scalar`] or the kernel
//! [`galion_tensor::simd::dot_i8_for`] selects. There is no second SIMD
//! implementation. [`KeyArena::active_kernel`] reports the level that kernel will
//! actually run: the cargo feature, the CPU flag, and the AVX-512BW rule
//! inside galion-tensor. A GPU does not change this scan. The boot profile
//! that turns a card into an API model is a different program (`galion-boot`);
//! this crate does not read it.
//!
//! `KEY_DIM` is the 32-wide key from the R4 layout. The header stores `k`,
//! so a wider key (the 256-wide width in the R1 note) is the same file.
//! This version stores no per-key scale. The compare is the i32 dot.
//!
//! The retrieval thread is not linked. Nothing here opens FTS, a socket, or
//! a card directory.

#![forbid(unsafe_code)]

use galion_tensor::features::{self, SimdLevel};
use galion_tensor::gq::{MappedBytes, Prefault};
use galion_tensor::simd::{dot_i8_for, dot_i8_level, i8_from_bytes, DOT_I8_MAX_K};
use std::fmt;
use std::path::Path;

pub use galion_tensor::gq::Prefault as KeyPrefault;

/// Magic `HKB1` — HORBM key arena, version 1.
pub const MAGIC: [u8; 4] = *b"HKB1";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 64;
pub const ALIGN: usize = 64;
/// Hot-path key width from the R4 layout. Files may store another `k`.
pub const KEY_DIM: usize = 32;

/// Fixed-seed pack used by the bit-exact gate. Row 0 is consent-denied.
pub const FIXTURE_SEED: u64 = 0xF3F3_0001_484B_4231;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaError {
    Io(String),
    Map(String),
    BadMagic,
    UnsupportedVersion(u16),
    BadFlags(u16),
    Truncated {
        need: usize,
        have: usize,
    },
    BadShape {
        n: usize,
        k: usize,
    },
    BadLayout,
    BadConsent,
    LenMismatch {
        field: &'static str,
        got: usize,
        expect: usize,
    },
    QueryLen {
        got: usize,
        k: usize,
    },
    OutLen {
        got: usize,
        need: usize,
    },
}

impl fmt::Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "horbm-keys: {self:?}")
    }
}

impl std::error::Error for ArenaError {}

/// Byte plan for `n` keys of width `k`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub n: usize,
    pub k: usize,
    pub keys_off: usize,
    pub keys_len: usize,
    pub side_off: usize,
    pub clause_off: usize,
    pub consent_off: usize,
    pub consent_len: usize,
    pub source_off: usize,
    pub file_len: usize,
}

pub fn layout(n: usize, k: usize) -> Result<Layout, ArenaError> {
    if k == 0 || k > DOT_I8_MAX_K || u32::try_from(n).is_err() || u32::try_from(k).is_err() {
        return Err(ArenaError::BadShape { n, k });
    }
    let keys_len = n.checked_mul(k).ok_or(ArenaError::BadShape { n, k })?;
    let keys_off = HEADER_LEN;
    let keys_end = keys_off + keys_len;
    let side_off = (keys_end + (ALIGN - 1)) & !(ALIGN - 1);
    let clause_off = side_off;
    let clause_len = n.checked_mul(8).ok_or(ArenaError::BadShape { n, k })?;
    let consent_off = clause_off + clause_len;
    let consent_len = n.div_ceil(8);
    let source_off = (consent_off + consent_len + 7) & !7;
    let source_len = n.checked_mul(8).ok_or(ArenaError::BadShape { n, k })?;
    let file_len = source_off + source_len;
    Ok(Layout {
        n,
        k,
        keys_off,
        keys_len,
        side_off,
        clause_off,
        consent_off,
        consent_len,
        source_off,
        file_len,
    })
}

/// One pack: row-major int8 keys plus the side tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPack {
    pub k: usize,
    pub keys: Vec<i8>,
    pub clause_id: Vec<u64>,
    pub allow: Vec<bool>,
    pub source: Vec<u64>,
}

impl KeyPack {
    pub fn n(&self) -> usize {
        self.clause_id.len()
    }

    pub fn encode(&self) -> Result<Vec<u8>, ArenaError> {
        let n = self.n();
        let expect = n
            .checked_mul(self.k)
            .ok_or(ArenaError::BadShape { n, k: self.k })?;
        if self.keys.len() != expect {
            return Err(ArenaError::LenMismatch {
                field: "keys",
                got: self.keys.len(),
                expect,
            });
        }
        if self.allow.len() != n {
            return Err(ArenaError::LenMismatch {
                field: "allow",
                got: self.allow.len(),
                expect: n,
            });
        }
        if self.source.len() != n {
            return Err(ArenaError::LenMismatch {
                field: "source",
                got: self.source.len(),
                expect: n,
            });
        }
        let lay = layout(n, self.k)?;
        let mut buf = vec![0u8; lay.file_len];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4..6].copy_from_slice(&VERSION.to_le_bytes());
        buf[8..12].copy_from_slice(&(n as u32).to_le_bytes());
        buf[12..16].copy_from_slice(&(self.k as u32).to_le_bytes());
        buf[16..20].copy_from_slice(&(lay.keys_off as u32).to_le_bytes());
        buf[20..24].copy_from_slice(&(lay.side_off as u32).to_le_bytes());
        for (i, &q) in self.keys.iter().enumerate() {
            buf[lay.keys_off + i] = q as u8;
        }
        for (i, &id) in self.clause_id.iter().enumerate() {
            let o = lay.clause_off + i * 8;
            buf[o..o + 8].copy_from_slice(&id.to_le_bytes());
        }
        for (i, &on) in self.allow.iter().enumerate() {
            if on {
                buf[lay.consent_off + i / 8] |= 1 << (i % 8);
            }
        }
        for (i, &id) in self.source.iter().enumerate() {
            let o = lay.source_off + i * 8;
            buf[o..o + 8].copy_from_slice(&id.to_le_bytes());
        }
        Ok(buf)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), ArenaError> {
        let bytes = self.encode()?;
        std::fs::write(path, bytes).map_err(|e| ArenaError::Io(e.to_string()))
    }
}

struct Copy64 {
    buf: Vec<u8>,
    start: usize,
    len: usize,
}

/// Read-only arena. Built at open. The retrieval thread is a later caller.
pub struct KeyArena {
    map: MappedBytes,
    lay: Layout,
    copied: Option<Copy64>,
}

impl fmt::Debug for KeyArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyArena")
            .field("n", &self.lay.n)
            .field("k", &self.lay.k)
            .field("copied", &self.copied.is_some())
            .field("kernel", &Self::active_kernel().name())
            .finish()
    }
}

impl KeyArena {
    /// Map `path` and prefault it before the header is checked.
    pub fn open(path: impl AsRef<Path>, pf: Prefault) -> Result<Self, ArenaError> {
        let map = MappedBytes::open(path, pf).map_err(|e| ArenaError::Map(e.to_string()))?;
        Self::from_map(map)
    }

    fn from_map(map: MappedBytes) -> Result<Self, ArenaError> {
        let bytes = map.bytes();
        if bytes.len() < HEADER_LEN {
            return Err(ArenaError::Truncated {
                need: HEADER_LEN,
                have: bytes.len(),
            });
        }
        if bytes[0..4] != MAGIC {
            return Err(ArenaError::BadMagic);
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(ArenaError::UnsupportedVersion(version));
        }
        let flags = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        if flags != 0 {
            return Err(ArenaError::BadFlags(flags));
        }
        if bytes[24..HEADER_LEN].iter().any(|&b| b != 0) {
            return Err(ArenaError::BadLayout);
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let k = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let keys_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        let side_off = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let lay = layout(n, k)?;
        if keys_off != lay.keys_off || side_off != lay.side_off || bytes.len() != lay.file_len {
            return Err(ArenaError::BadLayout);
        }
        if lay.consent_len > 0 && lay.n % 8 != 0 {
            let used = lay.n % 8;
            let mask = !((1u8 << used) - 1);
            if bytes[lay.consent_off + lay.consent_len - 1] & mask != 0 {
                return Err(ArenaError::BadConsent);
            }
        }
        let copied = align_keys(bytes, lay.keys_off, lay.keys_len);
        Ok(KeyArena { map, lay, copied })
    }

    pub fn len(&self) -> usize {
        self.lay.n
    }

    pub fn is_empty(&self) -> bool {
        self.lay.n == 0
    }

    pub fn key_dim(&self) -> usize {
        self.lay.k
    }

    pub fn layout(&self) -> Layout {
        self.lay
    }

    /// Level `dot_i8_for(selected())` will run on this CPU and this build.
    pub fn active_kernel() -> SimdLevel {
        dot_i8_level(features::selected())
    }

    pub fn keys_aligned_64(&self) -> bool {
        self.lay.keys_len == 0 || (self.keys_i8().as_ptr() as usize) % ALIGN == 0
    }

    pub fn keys_copied(&self) -> bool {
        self.copied.is_some()
    }

    pub fn clause_id(&self, i: usize) -> Result<u64, ArenaError> {
        self.u64_at(self.lay.clause_off, i)
    }

    pub fn source(&self, i: usize) -> Result<u64, ArenaError> {
        self.u64_at(self.lay.source_off, i)
    }

    pub fn allows(&self, i: usize) -> Result<bool, ArenaError> {
        if i >= self.lay.n {
            return Err(ArenaError::LenMismatch {
                field: "row",
                got: i,
                expect: self.lay.n,
            });
        }
        let b = self.map.bytes()[self.lay.consent_off + i / 8];
        Ok((b >> (i % 8)) & 1 == 1)
    }

    /// Integer dot of every row. Consent is ignored. `out.len()` must be `n`.
    pub fn dots_into(
        &self,
        query: &[i8],
        level: SimdLevel,
        out: &mut [i32],
    ) -> Result<(), ArenaError> {
        self.check_query(query)?;
        if out.len() != self.lay.n {
            return Err(ArenaError::OutLen {
                got: out.len(),
                need: self.lay.n,
            });
        }
        let dot = dot_i8_for(level);
        let keys = self.keys_i8();
        let k = self.lay.k;
        for (i, slot) in out.iter_mut().enumerate() {
            let s = i * k;
            *slot = dot(&keys[s..s + k], query);
        }
        Ok(())
    }

    pub fn dots_scalar_into(&self, query: &[i8], out: &mut [i32]) -> Result<(), ArenaError> {
        self.dots_into(query, SimdLevel::Scalar, out)
    }

    pub fn dots_selected_into(&self, query: &[i8], out: &mut [i32]) -> Result<(), ArenaError> {
        self.dots_into(query, features::selected(), out)
    }

    /// Dots only consent-allowed rows, in row order. A denied row is not dotted.
    /// Returns how many scores were written. `out` must hold at least that many.
    pub fn dots_allowed_into(
        &self,
        query: &[i8],
        level: SimdLevel,
        out: &mut [i32],
    ) -> Result<usize, ArenaError> {
        self.check_query(query)?;
        let dot = dot_i8_for(level);
        let keys = self.keys_i8();
        let k = self.lay.k;
        let mut w = 0usize;
        for i in 0..self.lay.n {
            if !self.allows(i)? {
                continue;
            }
            if w >= out.len() {
                return Err(ArenaError::OutLen {
                    got: out.len(),
                    need: w + 1,
                });
            }
            let s = i * k;
            out[w] = dot(&keys[s..s + k], query);
            w += 1;
        }
        Ok(w)
    }

    fn check_query(&self, query: &[i8]) -> Result<(), ArenaError> {
        if query.len() != self.lay.k {
            return Err(ArenaError::QueryLen {
                got: query.len(),
                k: self.lay.k,
            });
        }
        Ok(())
    }

    fn u64_at(&self, base: usize, i: usize) -> Result<u64, ArenaError> {
        if i >= self.lay.n {
            return Err(ArenaError::LenMismatch {
                field: "row",
                got: i,
                expect: self.lay.n,
            });
        }
        let o = base + i * 8;
        let b = self
            .map
            .bytes()
            .get(o..o + 8)
            .ok_or(ArenaError::BadLayout)?;
        Ok(u64::from_le_bytes(b.try_into().unwrap()))
    }

    fn keys_i8(&self) -> &[i8] {
        let bytes = match &self.copied {
            Some(c) => &c.buf[c.start..c.start + c.len],
            None => {
                let o = self.lay.keys_off;
                let n = self.lay.keys_len;
                &self.map.bytes()[o..o + n]
            }
        };
        i8_from_bytes(bytes)
    }
}

fn align_keys(bytes: &[u8], off: usize, len: usize) -> Option<Copy64> {
    if len == 0 {
        return None;
    }
    let src = &bytes[off..off + len];
    if (src.as_ptr() as usize) % ALIGN == 0 {
        return None;
    }
    let mut buf = vec![0u8; len + ALIGN];
    let addr = buf.as_ptr() as usize;
    let start = (ALIGN - (addr % ALIGN)) % ALIGN;
    buf[start..start + len].copy_from_slice(src);
    Some(Copy64 { buf, start, len })
}

/// Eight keys of width [`KEY_DIM`], from [`FIXTURE_SEED`]. Row 0 is denied.
pub fn fixed_fixture() -> (KeyPack, Vec<i8>) {
    let mut state = FIXTURE_SEED;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state
    };
    let n = 8;
    let k = KEY_DIM;
    let keys = (0..n * k).map(|_| (next() >> 33) as u8 as i8).collect();
    let query = (0..k).map(|_| (next() >> 33) as u8 as i8).collect();
    let clause_id = (0..n as u64).map(|i| 0x1000 + i).collect();
    let mut allow = vec![true; n];
    allow[0] = false;
    let source = (0..n as u64).map(|i| 0xA000 + i).collect();
    (
        KeyPack {
            k,
            keys,
            clause_id,
            allow,
            source,
        },
        query,
    )
}

/// Deterministic pack of any shape. Every row is allowed.
pub fn shaped_pack(n: usize, k: usize, seed: u64) -> (KeyPack, Vec<i8>) {
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state
    };
    let keys = (0..n * k).map(|_| (next() >> 33) as u8 as i8).collect();
    let query = (0..k).map(|_| (next() >> 33) as u8 as i8).collect();
    let clause_id = (0..n as u64).collect();
    let allow = vec![true; n];
    let source = (0..n as u64).map(|i| i.wrapping_add(7)).collect();
    (
        KeyPack {
            k,
            keys,
            clause_id,
            allow,
            source,
        },
        query,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_pads_keys_to_64_and_keeps_side_tables_off_the_key_span() {
        let lay = layout(8, KEY_DIM).unwrap();
        assert_eq!(lay.keys_off, 64);
        assert_eq!(lay.keys_off % 64, 0);
        assert_eq!(lay.keys_len, 256);
        assert!(lay.side_off >= lay.keys_off + lay.keys_len);
        assert_eq!(lay.side_off % 64, 0);
        assert_eq!(lay.file_len, 456);
        let odd = layout(3, 7).unwrap();
        assert_eq!(odd.keys_len, 21);
        assert_eq!(odd.consent_len, 1);
        assert!(odd.side_off % 64 == 0);
        assert!(layout(1, 0).is_err());
        assert!(layout(1, DOT_I8_MAX_K + 1).is_err());
        let empty = layout(0, KEY_DIM).unwrap();
        assert_eq!(empty.file_len, HEADER_LEN);
    }
}
