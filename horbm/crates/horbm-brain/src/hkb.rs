// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Versioned `HKB1` file with the string table and a per-key int8 scale.
//!
//! Version 1 is the layout [`horbm_keys`] already writes: 64-byte header,
//! int8 keys, `u64` clause ids, a consent bitset, `u64` source ids. That
//! file still loads here. It has no string table. Every string id misses,
//! and each row's scale is `1`.
//!
//! Version 2 ([`STORED_VERSION`]) keeps that region and appends `n` int8
//! scales and one UTF-8 `item_id` per row. The dot is
//! [`galion_tensor::simd::dot_i8_for`]. There is no second kernel. The
//! dequantised score is [`dequant_score`]: the i32 dot times the row's int8
//! scale, in `f32`, the same expression on every level.
//!
//! A consent-denied row and a missing id return `None`. The denied row is
//! not dotted.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub use galion_tensor::features::SimdLevel;
pub use horbm_keys::KeyPrefault;

use galion_tensor::features;
use galion_tensor::gq::MappedBytes;
use galion_tensor::simd::{dot_i8_for, dot_i8_level};
use horbm_keys::{encode_query_i8, layout, Layout, HEADER_LEN, MAGIC};

/// `HKB1` version that stores the string table and the per-key scales.
pub const STORED_VERSION: u16 = 2;

const SCALE_OFF_AT: usize = 24;
const STRINGS_OFF_AT: usize = 28;
const STRINGS_LEN_AT: usize = 32;
const RESERVED_AT: usize = 36;

/// Dequantised score of one int8 dot.
///
/// `dot` is the i32 sum from the scalar reference or from the SIMD kernel,
/// which match. `scale` is the per-key int8 scale. The real value of a
/// stored lane is `quantised_lane * scale`, so the dot with an int8 query
/// (query scale 1) is `dot * scale`. This is that product in `f32`:
///
/// ```text
/// (dot as f32) * (scale as f32)
/// ```
///
/// It is the same expression at every kernel. When the integer product is
/// an exact `f32`, the score is that integer.
#[inline]
pub const fn dequant_score(dot: i32, scale: i8) -> f32 {
    (dot as f32) * (scale as f32)
}

/// Why a file or a score was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HkbError {
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
    BadStringTable,
    EmptyItemId,
    DuplicateItemId,
    LenMismatch {
        field: &'static str,
        got: usize,
        expect: usize,
    },
    QueryLen {
        got: usize,
        k: usize,
    },
}

impl fmt::Display for HkbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "horbm-brain hkb: {self:?}")
    }
}

impl std::error::Error for HkbError {}

/// One version-2 pack. [`StoredPack::encode`] writes the file.
/// [`StoredKeys`] reads version 1 and version 2.
#[derive(Debug, Clone)]
pub struct StoredPack {
    pub k: usize,
    pub keys: Vec<i8>,
    pub clause_id: Vec<u64>,
    pub allow: Vec<bool>,
    pub source: Vec<u64>,
    pub item_id: Vec<String>,
    pub scale: Vec<i8>,
}

impl StoredPack {
    pub fn n(&self) -> usize {
        self.clause_id.len()
    }

    pub fn encode(&self) -> Result<Vec<u8>, HkbError> {
        let n = self.n();
        same_len("allow", self.allow.len(), n)?;
        same_len("source", self.source.len(), n)?;
        same_len("item_id", self.item_id.len(), n)?;
        same_len("scale", self.scale.len(), n)?;
        let lay = map_layout(n, self.k)?;
        same_len("keys", self.keys.len(), lay.keys_len)?;
        let strings = string_table(&self.item_id)?;
        index_ids(&self.item_id)?;
        let scale_off = lay.file_len;
        let strings_off = scale_off.checked_add(n).ok_or(HkbError::BadLayout)?;
        let strings_len = strings.len();
        let total = strings_off
            .checked_add(strings_len)
            .ok_or(HkbError::BadLayout)?;
        let mut buf = vec![0u8; total];
        buf[0..4].copy_from_slice(&MAGIC);
        write_u16(&mut buf, 4, STORED_VERSION);
        write_u32(&mut buf, 8, u32_of(n)?);
        write_u32(&mut buf, 12, u32_of(self.k)?);
        write_u32(&mut buf, 16, u32_of(lay.keys_off)?);
        write_u32(&mut buf, 20, u32_of(lay.side_off)?);
        write_u32(&mut buf, SCALE_OFF_AT, u32_of(scale_off)?);
        write_u32(&mut buf, STRINGS_OFF_AT, u32_of(strings_off)?);
        write_u32(&mut buf, STRINGS_LEN_AT, u32_of(strings_len)?);
        for (i, &q) in self.keys.iter().enumerate() {
            buf[lay.keys_off + i] = q as u8;
        }
        write_u64s(&mut buf, lay.clause_off, &self.clause_id);
        for (i, &on) in self.allow.iter().enumerate() {
            if on {
                buf[lay.consent_off + i / 8] |= 1 << (i % 8);
            }
        }
        write_u64s(&mut buf, lay.source_off, &self.source);
        for (i, &scale) in self.scale.iter().enumerate() {
            buf[scale_off + i] = scale as u8;
        }
        buf[strings_off..strings_off + strings_len].copy_from_slice(&strings);
        Ok(buf)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), HkbError> {
        let bytes = self.encode()?;
        std::fs::write(path, bytes).map_err(|err| HkbError::Io(err.to_string()))
    }
}

/// Read-only arena. Built from bytes or from a prefaulted map.
pub struct StoredKeys {
    version: u16,
    k: usize,
    keys: Vec<i8>,
    clause_id: Vec<u64>,
    allow: Vec<bool>,
    source: Vec<u64>,
    scale: Vec<i8>,
    item_id: Vec<String>,
    index: HashMap<String, usize>,
}

impl fmt::Debug for StoredKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredKeys")
            .field("version", &self.version)
            .field("n", &self.len())
            .field("k", &self.k)
            .field("kernel", &Self::active_kernel().name())
            .finish()
    }
}

impl StoredKeys {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HkbError> {
        if bytes.len() < HEADER_LEN {
            return Err(HkbError::Truncated {
                need: HEADER_LEN,
                have: bytes.len(),
            });
        }
        if bytes[0..4] != MAGIC {
            return Err(HkbError::BadMagic);
        }
        let version = read_u16(bytes, 4)?;
        if version != horbm_keys::VERSION && version != STORED_VERSION {
            return Err(HkbError::UnsupportedVersion(version));
        }
        let flags = read_u16(bytes, 6)?;
        if flags != 0 {
            return Err(HkbError::BadFlags(flags));
        }
        let n = read_u32(bytes, 8)? as usize;
        let k = read_u32(bytes, 12)? as usize;
        let keys_off = read_u32(bytes, 16)? as usize;
        let side_off = read_u32(bytes, 20)? as usize;
        let lay = map_layout(n, k)?;
        if keys_off != lay.keys_off || side_off != lay.side_off {
            return Err(HkbError::BadLayout);
        }
        if version == horbm_keys::VERSION {
            parse_v1(bytes, lay)
        } else {
            parse_v2(bytes, lay)
        }
    }

    /// Map `path` and prefault it, then parse. The owned copy is what later
    /// scores use. Unix and Windows both go through [`MappedBytes`].
    pub fn open(path: impl AsRef<Path>, prefault: KeyPrefault) -> Result<Self, HkbError> {
        let mapped =
            MappedBytes::open(path, prefault).map_err(|err| HkbError::Map(err.to_string()))?;
        Self::from_bytes(mapped.bytes())
    }

    pub fn version(&self) -> u16 {
        self.version
    }

    pub fn len(&self) -> usize {
        self.clause_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clause_id.is_empty()
    }

    pub fn key_dim(&self) -> usize {
        self.k
    }

    /// Level `dot_i8_for(selected())` will run on this CPU and this build.
    pub fn active_kernel() -> SimdLevel {
        dot_i8_level(features::selected())
    }

    pub fn clause_id(&self, row: usize) -> Result<u64, HkbError> {
        self.clause_id.get(row).copied().ok_or(self.row_err(row))
    }

    pub fn source(&self, row: usize) -> Result<u64, HkbError> {
        self.source.get(row).copied().ok_or(self.row_err(row))
    }

    pub fn allows(&self, row: usize) -> Result<bool, HkbError> {
        self.allow.get(row).copied().ok_or(self.row_err(row))
    }

    pub fn scale(&self, row: usize) -> Result<i8, HkbError> {
        self.scale.get(row).copied().ok_or(self.row_err(row))
    }

    /// `Ok(None)` on a version-1 row: the file has no string table.
    pub fn item_id(&self, row: usize) -> Result<Option<&str>, HkbError> {
        if row >= self.len() {
            return Err(self.row_err(row));
        }
        Ok(self.item_id.get(row).map(String::as_str))
    }

    /// Row of a version-2 id. `None` when the id is absent. A version-1 file
    /// has an empty index, so every id is `None`.
    pub fn row_of(&self, item_id: &str) -> Option<usize> {
        self.index.get(item_id).copied()
    }

    pub fn row(&self, index: usize) -> Result<&[i8], HkbError> {
        if index >= self.len() {
            return Err(self.row_err(index));
        }
        let start = index * self.k;
        Ok(&self.keys[start..start + self.k])
    }

    /// Dequantised score of one row. `Ok(None)` when the row is denied.
    /// A denied row is not dotted, and a short query on a denied row is
    /// still `None`.
    pub fn score_row_at(
        &self,
        row: usize,
        query: &[i8],
        level: SimdLevel,
    ) -> Result<Option<f32>, HkbError> {
        if row >= self.len() {
            return Err(self.row_err(row));
        }
        if !self.allow[row] {
            return Ok(None);
        }
        if query.len() != self.k {
            return Err(HkbError::QueryLen {
                got: query.len(),
                k: self.k,
            });
        }
        Ok(Some(self.dequant_row(row, query, level)))
    }

    /// `None` when `item_id` is absent or the row is denied.
    pub fn score_query_at(
        &self,
        item_id: &str,
        query: &[i8],
        level: SimdLevel,
    ) -> Result<Option<f32>, HkbError> {
        let Some(&row) = self.index.get(item_id) else {
            return Ok(None);
        };
        self.score_row_at(row, query, level)
    }

    pub fn score_text_at(&self, item_id: &str, query: &str, level: SimdLevel) -> Option<f32> {
        let &row = self.index.get(item_id)?;
        if !self.allow[row] {
            return None;
        }
        let encoded = encode_query_i8(query, self.k);
        Some(self.dequant_row(row, &encoded, level))
    }

    pub fn score_scalar_text(&self, item_id: &str, query: &str) -> Option<f32> {
        self.score_text_at(item_id, query, SimdLevel::Scalar)
    }

    /// Same `f32` bits as [`Self::score_scalar_text`], through the selected kernel.
    pub fn score_selected_text(&self, item_id: &str, query: &str) -> Option<f32> {
        self.score_text_at(item_id, query, features::selected())
    }

    fn dequant_row(&self, row: usize, query: &[i8], level: SimdLevel) -> f32 {
        let start = row * self.k;
        let dot = dot_i8_for(level)(&self.keys[start..start + self.k], query);
        dequant_score(dot, self.scale[row])
    }

    fn row_err(&self, row: usize) -> HkbError {
        HkbError::LenMismatch {
            field: "row",
            got: row,
            expect: self.len(),
        }
    }
}

fn parse_v1(bytes: &[u8], lay: Layout) -> Result<StoredKeys, HkbError> {
    if bytes[SCALE_OFF_AT..HEADER_LEN].iter().any(|&b| b != 0) {
        return Err(HkbError::BadLayout);
    }
    if bytes.len() != lay.file_len {
        return Err(length_err(lay.file_len, bytes.len()));
    }
    let side = read_side(bytes, &lay)?;
    finish(horbm_keys::VERSION, lay.k, side, vec![1; lay.n], Vec::new())
}

fn parse_v2(bytes: &[u8], lay: Layout) -> Result<StoredKeys, HkbError> {
    if bytes[RESERVED_AT..HEADER_LEN].iter().any(|&b| b != 0) {
        return Err(HkbError::BadLayout);
    }
    let scale_off = read_u32(bytes, SCALE_OFF_AT)? as usize;
    let strings_off = read_u32(bytes, STRINGS_OFF_AT)? as usize;
    let strings_len = read_u32(bytes, STRINGS_LEN_AT)? as usize;
    let strings_end = strings_off
        .checked_add(strings_len)
        .ok_or(HkbError::BadLayout)?;
    if bytes.len() < strings_end {
        return Err(HkbError::Truncated {
            need: strings_end,
            have: bytes.len(),
        });
    }
    let scale_end = scale_off.checked_add(lay.n).ok_or(HkbError::BadLayout)?;
    if scale_off != lay.file_len || strings_off != scale_end || bytes.len() != strings_end {
        return Err(HkbError::BadLayout);
    }
    let side = read_side(bytes, &lay)?;
    let scale = bytes[scale_off..strings_off]
        .iter()
        .map(|&b| b as i8)
        .collect();
    let item_id = parse_strings(&bytes[strings_off..strings_end], lay.n)?;
    finish(STORED_VERSION, lay.k, side, scale, item_id)
}

struct Side {
    keys: Vec<i8>,
    clause_id: Vec<u64>,
    allow: Vec<bool>,
    source: Vec<u64>,
}

fn finish(
    version: u16,
    k: usize,
    side: Side,
    scale: Vec<i8>,
    item_id: Vec<String>,
) -> Result<StoredKeys, HkbError> {
    let index = index_ids(&item_id)?;
    Ok(StoredKeys {
        version,
        k,
        keys: side.keys,
        clause_id: side.clause_id,
        allow: side.allow,
        source: side.source,
        scale,
        item_id,
        index,
    })
}

fn read_side(bytes: &[u8], lay: &Layout) -> Result<Side, HkbError> {
    if bytes.len() < lay.file_len {
        return Err(HkbError::Truncated {
            need: lay.file_len,
            have: bytes.len(),
        });
    }
    if lay.consent_len > 0 && lay.n % 8 != 0 {
        let used = lay.n % 8;
        let mask = !((1u8 << used) - 1);
        if bytes[lay.consent_off + lay.consent_len - 1] & mask != 0 {
            return Err(HkbError::BadConsent);
        }
    }
    let keys = bytes[lay.keys_off..lay.keys_off + lay.keys_len]
        .iter()
        .map(|&b| b as i8)
        .collect();
    Ok(Side {
        keys,
        clause_id: read_u64s(bytes, lay.clause_off, lay.n),
        allow: (0..lay.n)
            .map(|i| {
                let b = bytes[lay.consent_off + i / 8];
                (b >> (i % 8)) & 1 == 1
            })
            .collect(),
        source: read_u64s(bytes, lay.source_off, lay.n),
    })
}

fn parse_strings(bytes: &[u8], n: usize) -> Result<Vec<String>, HkbError> {
    let mut off = 0usize;
    let mut ids = Vec::with_capacity(n);
    for _ in 0..n {
        if bytes.len().saturating_sub(off) < 2 {
            return Err(HkbError::BadStringTable);
        }
        let len = read_u16(bytes, off)? as usize;
        off += 2;
        if len == 0 {
            return Err(HkbError::EmptyItemId);
        }
        if bytes.len().saturating_sub(off) < len {
            return Err(HkbError::BadStringTable);
        }
        let raw = &bytes[off..off + len];
        off += len;
        let text = std::str::from_utf8(raw).map_err(|_| HkbError::BadStringTable)?;
        ids.push(text.to_string());
    }
    if off != bytes.len() {
        return Err(HkbError::BadStringTable);
    }
    index_ids(&ids)?;
    Ok(ids)
}

fn string_table(ids: &[String]) -> Result<Vec<u8>, HkbError> {
    let mut out = Vec::new();
    for id in ids {
        if id.is_empty() {
            return Err(HkbError::EmptyItemId);
        }
        let len = u16::try_from(id.len()).map_err(|_| HkbError::BadStringTable)?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(id.as_bytes());
    }
    Ok(out)
}

fn index_ids(ids: &[String]) -> Result<HashMap<String, usize>, HkbError> {
    let mut index = HashMap::with_capacity(ids.len());
    for (row, id) in ids.iter().enumerate() {
        if id.is_empty() {
            return Err(HkbError::EmptyItemId);
        }
        if index.insert(id.clone(), row).is_some() {
            return Err(HkbError::DuplicateItemId);
        }
    }
    Ok(index)
}

fn map_layout(n: usize, k: usize) -> Result<Layout, HkbError> {
    layout(n, k).map_err(|_| HkbError::BadShape { n, k })
}

fn same_len(field: &'static str, got: usize, expect: usize) -> Result<(), HkbError> {
    if got == expect {
        Ok(())
    } else {
        Err(HkbError::LenMismatch { field, got, expect })
    }
}

fn length_err(need: usize, have: usize) -> HkbError {
    if have < need {
        HkbError::Truncated { need, have }
    } else {
        HkbError::BadLayout
    }
}

fn u32_of(value: usize) -> Result<u32, HkbError> {
    u32::try_from(value).map_err(|_| HkbError::BadLayout)
}

fn write_u16(buf: &mut [u8], off: usize, value: u16) {
    buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], off: usize, value: u32) {
    buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64s(buf: &mut [u8], off: usize, values: &[u64]) {
    for (i, &value) in values.iter().enumerate() {
        let start = off + i * 8;
        buf[start..start + 8].copy_from_slice(&value.to_le_bytes());
    }
}

fn read_u16(bytes: &[u8], off: usize) -> Result<u16, HkbError> {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(bytes.get(off..off + 2).ok_or(HkbError::Truncated {
        need: off + 2,
        have: bytes.len(),
    })?);
    Ok(u16::from_le_bytes(buf))
}

fn read_u32(bytes: &[u8], off: usize) -> Result<u32, HkbError> {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(bytes.get(off..off + 4).ok_or(HkbError::Truncated {
        need: off + 4,
        have: bytes.len(),
    })?);
    Ok(u32::from_le_bytes(buf))
}

fn read_u64s(bytes: &[u8], off: usize, n: usize) -> Vec<u64> {
    (0..n)
        .map(|i| {
            let start = off + i * 8;
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[start..start + 8]);
            u64::from_le_bytes(buf)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_tensor::features;
    use galion_tensor::simd::DOT_I8_MAX_K;
    use horbm_keys::{
        encode_query_i8, layout, shaped_pack, KeyArena, KeyPack, HEADER_LEN as V1_HEADER, KEY_DIM,
        MAGIC,
    };

    fn bits(score: Option<f32>) -> Option<u32> {
        score.map(f32::to_bits)
    }

    fn reference(row: &[i8], query: &[i8], scale: i8) -> f32 {
        let mut acc = 0i32;
        for (&a, &b) in row.iter().zip(query) {
            acc = acc.wrapping_add(i32::from(a) * i32::from(b));
        }
        dequant_score(acc, scale)
    }

    fn scales_for(n: usize) -> Vec<i8> {
        (0..n)
            .map(|i| match i % 5 {
                0 => 1,
                1 => -3,
                2 => 0,
                3 => 127,
                _ => -128,
            })
            .collect()
    }

    fn stored_from_shape(n: usize, k: usize, seed: u64) -> (StoredPack, Vec<i8>) {
        let (mut pack, query) = shaped_pack(n, k, seed);
        if n > 0 {
            pack.allow[0] = false;
        }
        let item_id = (0..n).map(|i| format!("row-{i}")).collect();
        let stored = StoredPack {
            k: pack.k,
            keys: pack.keys,
            clause_id: pack.clause_id,
            allow: pack.allow,
            source: pack.source,
            item_id,
            scale: scales_for(n),
        };
        (stored, query)
    }

    fn assert_levels(keys: &StoredKeys, id: &str, query: &[i8], expect: Option<f32>) {
        let mut padded = vec![0i8; query.len() + 3];
        padded[1..1 + query.len()].copy_from_slice(query);
        let unaligned = &padded[1..1 + query.len()];
        for level in [
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Avx512,
            SimdLevel::Neon,
        ] {
            assert_eq!(
                bits(keys.score_query_at(id, query, level).unwrap()),
                bits(expect),
                "{id} {level:?}"
            );
            assert_eq!(
                bits(keys.score_query_at(id, unaligned, level).unwrap()),
                bits(expect),
                "{id} unaligned {level:?}"
            );
        }
    }

    #[test]
    fn an_integer_scale_reconstructs_the_real_dot() {
        let scale: i8 = -3;
        let quant = [4i8, 1, -2];
        let query = [2i8, -5, 7];
        let real: i64 = quant
            .iter()
            .zip(query)
            .map(|(&a, b)| i64::from(a) * i64::from(scale) * i64::from(b))
            .sum();
        assert_eq!(real, 33);
        assert_eq!(
            reference(&quant, &query, scale).to_bits(),
            (real as f32).to_bits()
        );
        let pack = StoredPack {
            k: 3,
            keys: quant.to_vec(),
            clause_id: vec![9],
            allow: vec![true],
            source: vec![4],
            item_id: vec!["row-0".to_string()],
            scale: vec![scale],
        };
        let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
        assert_levels(&keys, "row-0", &query, Some(real as f32));
    }

    #[test]
    fn version_1_bytes_still_load_with_unit_scale_and_no_strings() {
        assert_eq!(horbm_keys::VERSION, 1);
        assert_eq!(STORED_VERSION, 2);
        let (mut pack, query) = shaped_pack(5, 7, 0x51);
        pack.allow[0] = false;
        pack.allow[2] = false;
        let bytes = pack.encode().unwrap();
        let keys = StoredKeys::from_bytes(&bytes).unwrap();
        assert_eq!(keys.version(), 1);
        assert_eq!(keys.len(), 5);
        assert_eq!(keys.key_dim(), 7);
        assert_eq!(keys.clause_id(3).unwrap(), pack.clause_id[3]);
        assert_eq!(keys.source(4).unwrap(), pack.source[4]);
        for i in 0..5 {
            assert_eq!(keys.allows(i).unwrap(), pack.allow[i]);
            assert_eq!(keys.scale(i).unwrap(), 1);
            assert_eq!(keys.item_id(i).unwrap(), None);
            assert_eq!(keys.row(i).unwrap(), &pack.keys[i * 7..(i + 1) * 7]);
            let score = keys.score_row_at(i, &query, SimdLevel::Scalar).unwrap();
            if pack.allow[i] {
                assert_eq!(
                    bits(score),
                    bits(Some(reference(keys.row(i).unwrap(), &query, 1)))
                );
            } else {
                assert_eq!(score, None);
                assert_eq!(keys.score_row_at(i, &[], SimdLevel::Avx512).unwrap(), None);
            }
        }
        assert_eq!(keys.score_scalar_text("row-1", "q-lane"), None);
        assert_eq!(keys.score_selected_text("missing", "q-lane"), None);
        assert!(keys.score_row_at(5, &query, SimdLevel::Scalar).is_err());
    }

    #[test]
    fn version_2_stores_strings_and_scales_and_shares_the_version_1_region() {
        let (pack, _) = stored_from_shape(4, KEY_DIM, 0x52);
        let mut named = pack;
        named.item_id[3] = "id-β".to_string();
        let v2 = named.encode().unwrap();
        let v1 = KeyPack {
            k: named.k,
            keys: named.keys.clone(),
            clause_id: named.clause_id.clone(),
            allow: named.allow.clone(),
            source: named.source.clone(),
        }
        .encode()
        .unwrap();
        let lay = layout(named.n(), named.k).unwrap();
        assert_eq!(&v2[0..4], b"HKB1");
        assert_eq!(
            u16::from_le_bytes(v2[4..6].try_into().unwrap()),
            STORED_VERSION
        );
        assert_eq!(&v2[lay.keys_off..lay.file_len], &v1[lay.keys_off..]);
        assert!(v1[SCALE_OFF_AT..V1_HEADER].iter().all(|&b| b == 0));
        assert!(v2[RESERVED_AT..V1_HEADER].iter().all(|&b| b == 0));
        let keys = StoredKeys::from_bytes(&v2).unwrap();
        assert_eq!(keys.version(), 2);
        assert_eq!(keys.item_id(3).unwrap(), Some("id-β"));
        assert_eq!(keys.item_id(0).unwrap(), Some("row-0"));
        assert_eq!(keys.scale(1).unwrap(), -3);
        assert_eq!(keys.scale(2).unwrap(), 0);
        assert_eq!(keys.clause_id(2).unwrap(), named.clause_id[2]);
    }

    #[test]
    fn empty_version_1_and_version_2_files_both_load() {
        let v1 = KeyPack {
            k: KEY_DIM,
            keys: vec![],
            clause_id: vec![],
            allow: vec![],
            source: vec![],
        }
        .encode()
        .unwrap();
        assert_eq!(v1.len(), V1_HEADER);
        let old = StoredKeys::from_bytes(&v1).unwrap();
        assert!(old.is_empty());
        assert_eq!(old.version(), 1);
        assert_eq!(old.score_scalar_text("row-0", ""), None);
        let v2 = StoredPack {
            k: KEY_DIM,
            keys: vec![],
            clause_id: vec![],
            allow: vec![],
            source: vec![],
            item_id: vec![],
            scale: vec![],
        }
        .encode()
        .unwrap();
        assert_eq!(v2.len(), V1_HEADER);
        assert_ne!(v1, v2);
        let fresh = StoredKeys::from_bytes(&v2).unwrap();
        assert!(fresh.is_empty());
        assert_eq!(fresh.version(), 2);
        assert_eq!(fresh.score_selected_text("row-0", "q-lane"), None);
    }

    #[test]
    fn scores_match_the_scalar_reference_on_every_kernel() {
        let text = "q-lane";
        for (n, k, seed) in [
            (1usize, 1usize, 2u64),
            (4, 7, 3),
            (8, KEY_DIM, 4),
            (3, 33, 5),
            (2, 64, 6),
            (2, 65, 7),
            (4, 256, 9),
            (5, 15, 11),
        ] {
            let (pack, query) = stored_from_shape(n, k, seed);
            let bytes = pack.encode().unwrap();
            let keys = StoredKeys::from_bytes(&bytes).unwrap();
            let encoded = encode_query_i8(text, k);
            for i in 0..n {
                let id = format!("row-{i}");
                if pack.allow[i] {
                    let expect = reference(&pack.keys[i * k..(i + 1) * k], &query, pack.scale[i]);
                    assert_levels(&keys, &id, &query, Some(expect));
                    let text_expect =
                        reference(&pack.keys[i * k..(i + 1) * k], &encoded, pack.scale[i]);
                    assert_eq!(
                        bits(keys.score_scalar_text(&id, text)),
                        bits(Some(text_expect)),
                        "scalar text row {i} k {k}"
                    );
                    assert_eq!(
                        bits(keys.score_selected_text(&id, text)),
                        bits(keys.score_scalar_text(&id, text)),
                        "selected text row {i} k {k}"
                    );
                    for level in [SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon] {
                        assert_eq!(
                            bits(keys.score_text_at(&id, text, level)),
                            bits(Some(text_expect)),
                            "{level:?} text row {i} k {k}"
                        );
                    }
                } else {
                    assert_levels(&keys, &id, &query, None);
                    assert_eq!(keys.score_scalar_text(&id, text), None);
                    assert_eq!(keys.score_selected_text(&id, text), None);
                }
            }
            assert_eq!(keys.score_scalar_text("missing", text), None);
            assert_eq!(
                keys.score_query_at("missing", &query, SimdLevel::Avx2)
                    .unwrap(),
                None
            );
            assert_eq!(keys.score_selected_text("", text), None);
        }
    }

    #[test]
    fn selected_kernel_is_the_galion_tensor_detect() {
        let used = StoredKeys::active_kernel();
        let selected = features::selected();
        assert!(used <= selected);
        assert!(used.compiled() && features::cpu_supports(used));
        if !cfg!(any(
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-neon"
        )) {
            assert_eq!(used, SimdLevel::Scalar);
        }
        #[cfg(all(
            feature = "simd-avx2",
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        if features::cpu_supports(SimdLevel::Avx2) {
            assert!(
                used == SimdLevel::Avx2 || used == SimdLevel::Avx512,
                "compiled AVX on an AVX2 CPU must leave the scalar oracle, got {used:?}"
            );
        }
    }

    #[test]
    fn open_prefaults_and_version_2_is_not_a_version_1_arena() {
        let (pack, query) = stored_from_shape(3, 7, 99);
        let dir = std::env::temp_dir().join(format!("horbm-brain-hkb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.hkb");
        pack.write(&path).unwrap();
        for prefault in [KeyPrefault::None, KeyPrefault::Populate] {
            let keys = StoredKeys::open(&path, prefault).unwrap();
            assert_eq!(keys.version(), 2);
            assert_eq!(keys.score_scalar_text("missing", "q-lane"), None);
            assert!(keys
                .score_row_at(0, &query, SimdLevel::Scalar)
                .unwrap()
                .is_none());
            let allowed = keys.score_scalar_text("row-1", "q-lane").unwrap();
            let encoded = encode_query_i8("q-lane", 7);
            assert_eq!(
                allowed.to_bits(),
                reference(&pack.keys[7..14], &encoded, pack.scale[1]).to_bits()
            );
        }
        assert!(matches!(
            KeyArena::open(&path, KeyPrefault::Populate),
            Err(horbm_keys::ArenaError::UnsupportedVersion(2))
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_a_bad_header_a_dirty_consent_bit_and_a_bad_string_table() {
        assert!(matches!(
            StoredKeys::from_bytes(b"short"),
            Err(HkbError::Truncated { .. })
        ));
        let (pack, query) = stored_from_shape(3, 7, 99);
        let mut bytes = pack.encode().unwrap();
        bytes[0] = b'X';
        assert!(matches!(
            StoredKeys::from_bytes(&bytes),
            Err(HkbError::BadMagic)
        ));
        bytes[0] = MAGIC[0];
        bytes[4] = 9;
        assert!(matches!(
            StoredKeys::from_bytes(&bytes),
            Err(HkbError::UnsupportedVersion(9))
        ));
        bytes[4] = STORED_VERSION as u8;
        bytes[6] = 1;
        assert!(matches!(
            StoredKeys::from_bytes(&bytes),
            Err(HkbError::BadFlags(1))
        ));
        bytes[6] = 0;
        let good = pack.encode().unwrap();
        let mut dirty = good.clone();
        let lay = layout(3, 7).unwrap();
        dirty[lay.consent_off] |= 0b1000_0000;
        assert!(matches!(
            StoredKeys::from_bytes(&dirty),
            Err(HkbError::BadConsent)
        ));
        dirty = good.clone();
        dirty[super::RESERVED_AT] = 1;
        assert!(matches!(
            StoredKeys::from_bytes(&dirty),
            Err(HkbError::BadLayout)
        ));
        dirty = good.clone();
        dirty.push(0);
        assert!(matches!(
            StoredKeys::from_bytes(&dirty),
            Err(HkbError::BadLayout)
        ));
        assert!(matches!(
            StoredKeys::from_bytes(&good[..good.len() - 1]),
            Err(HkbError::Truncated { .. })
        ));

        let mut v1 = KeyPack {
            k: 7,
            keys: pack.keys.clone(),
            clause_id: pack.clause_id.clone(),
            allow: pack.allow.clone(),
            source: pack.source.clone(),
        }
        .encode()
        .unwrap();
        v1[super::SCALE_OFF_AT] = 1;
        assert!(matches!(
            StoredKeys::from_bytes(&v1),
            Err(HkbError::BadLayout)
        ));
        let mut v1_consent = KeyPack {
            k: 7,
            keys: pack.keys.clone(),
            clause_id: pack.clause_id.clone(),
            allow: pack.allow.clone(),
            source: pack.source.clone(),
        }
        .encode()
        .unwrap();
        v1_consent[lay.consent_off] |= 0b1000_0000;
        assert!(matches!(
            StoredKeys::from_bytes(&v1_consent),
            Err(HkbError::BadConsent)
        ));

        let mut dup = good.clone();
        let strings_off = u32::from_le_bytes(
            good[super::STRINGS_OFF_AT..super::STRINGS_OFF_AT + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        // "row-0" and "row-1" are the same length. Point the second id at the first.
        let first_len = 2 + "row-0".len();
        dup[strings_off + first_len + 2 + 4] = b'0';
        assert!(matches!(
            StoredKeys::from_bytes(&dup),
            Err(HkbError::DuplicateItemId)
        ));

        let beta = StoredPack {
            k: 1,
            keys: vec![0],
            clause_id: vec![1],
            allow: vec![true],
            source: vec![1],
            item_id: vec!["id-β".to_string()],
            scale: vec![1],
        }
        .encode()
        .unwrap();
        let mut utf = beta;
        let off = u32::from_le_bytes(
            utf[super::STRINGS_OFF_AT..super::STRINGS_OFF_AT + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let last = utf.len() - 1;
        assert!(last > off);
        utf[last] = 0xFF;
        assert!(matches!(
            StoredKeys::from_bytes(&utf),
            Err(HkbError::BadStringTable)
        ));

        let mut empty_id = StoredPack {
            k: 1,
            keys: vec![0],
            clause_id: vec![1],
            allow: vec![true],
            source: vec![1],
            item_id: vec!["ab".to_string()],
            scale: vec![1],
        }
        .encode()
        .unwrap();
        let soff = u32::from_le_bytes(
            empty_id[super::STRINGS_OFF_AT..super::STRINGS_OFF_AT + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        empty_id[soff] = 0;
        empty_id[soff + 1] = 0;
        empty_id.truncate(soff + 2);
        super::write_u32(&mut empty_id, super::STRINGS_LEN_AT, 2);
        assert!(matches!(
            StoredKeys::from_bytes(&empty_id),
            Err(HkbError::EmptyItemId)
        ));

        assert!(matches!(
            StoredPack {
                k: 1,
                keys: vec![0],
                clause_id: vec![1],
                allow: vec![true],
                source: vec![1],
                item_id: vec![String::new()],
                scale: vec![1],
            }
            .encode(),
            Err(HkbError::EmptyItemId)
        ));
        assert!(matches!(
            StoredPack {
                k: 1,
                keys: vec![0, 0],
                clause_id: vec![1, 2],
                allow: vec![true, true],
                source: vec![1, 2],
                item_id: vec!["same".into(), "same".into()],
                scale: vec![1, 1],
            }
            .encode(),
            Err(HkbError::DuplicateItemId)
        ));
        assert!(matches!(
            StoredPack {
                k: 0,
                keys: vec![],
                clause_id: vec![1],
                allow: vec![true],
                source: vec![1],
                item_id: vec!["a".into()],
                scale: vec![1],
            }
            .encode(),
            Err(HkbError::BadShape { .. })
        ));
        assert!(matches!(
            StoredPack {
                k: DOT_I8_MAX_K + 1,
                keys: vec![],
                clause_id: vec![1],
                allow: vec![true],
                source: vec![1],
                item_id: vec!["a".into()],
                scale: vec![1],
            }
            .encode(),
            Err(HkbError::BadShape { .. })
        ));
        let long = "x".repeat(65536);
        assert!(matches!(
            StoredPack {
                k: 1,
                keys: vec![1],
                clause_id: vec![1],
                allow: vec![true],
                source: vec![1],
                item_id: vec![long],
                scale: vec![1],
            }
            .encode(),
            Err(HkbError::BadStringTable)
        ));
        let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
        assert!(matches!(
            keys.score_query_at("row-1", &query[..query.len() - 1], SimdLevel::Scalar),
            Err(HkbError::QueryLen { .. })
        ));
        assert!(matches!(
            StoredKeys::open(dir_missing(), KeyPrefault::None),
            Err(HkbError::Map(_))
        ));
    }

    fn dir_missing() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("horbm-brain-hkb-missing-{}", std::process::id()))
    }
}
