// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Scan one query over the HKB1 arena and the person-diary arena.
//!
//! The retrieval thread in crate `horbm` still calls `search_lexical` and
//! does not call this. The call it would add is
//! [`DiaryScan::scan_selected_into`], described in
//! `docs/horbm/foundations/S54-diary-scan.md`. This module does not open
//! SQLite, does not link `horbm-memory`, and does not add a second SIMD
//! kernel. The dot is [`galion_tensor::simd::dot_i8_for`].
//!
//! After [`DiaryScan::open`], a query writes into the caller's hit buffer
//! and into scratch owned by the scan. It does not allocate.

use std::collections::HashMap;
use std::fmt;

use galion_tensor::features;
use galion_tensor::simd::dot_i8_for;

use crate::{dequant_score, SimdLevel, StoredKeys};

/// Diary width. Same lane count as `horbm-memory`'s `EMB_DIM`.
pub const DIARY_DIM: usize = 256;

/// Default top-k. Same cap as `horbm::MEMORY_HIT_CAP`.
pub const SCAN_CAP: usize = 4;

/// Cormack, Clarke, and Büttcher constant. Same value the diary uses when
/// it fuses two ranked lists.
pub const SCAN_RRF_K: f32 = 60.0;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const TOKEN_MAX: usize = 64;

/// Why a diary arena or an embedded query was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    Width { got: usize, expect: usize },
    EmptyItemId,
    DuplicateItemId,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "horbm-brain scan: {self:?}")
    }
}

impl std::error::Error for ScanError {}

/// Which arena a hit came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanPool {
    /// Person-diary hashing rows.
    Diary,
    /// `HKB1` rows.
    Keys,
}

/// One merged hit. `score` is the reciprocal-rank value. `native` is the
/// arena's own score: diary cosine, or the HKB1 dequantised dot.
///
/// [`DiaryScan::scan_selected_into`] writes `n` hits at the front of the
/// caller's buffer. The tail is untouched. [`Self::default`] is the filler
/// for that buffer, not a hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanHit {
    pub pool: ScanPool,
    pub row: u32,
    pub score: f32,
    pub native: f32,
}

impl Default for ScanHit {
    fn default() -> Self {
        Self {
            pool: ScanPool::Diary,
            row: 0,
            score: 0.0,
            native: 0.0,
        }
    }
}

/// Query epilogue. Keys use scale `1` and norm `1`. The diary stores the
/// quantised query's scale and reconstructed norm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedQuery {
    pub scale: f32,
    pub norm: f32,
}

/// Scale and norm of one diary embedding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiaryEmbed {
    pub scale: f32,
    pub norm: f32,
}

/// Raw int8 dot, the contract `horbm::Int8Scorer` names.
///
/// `score` is one id and returns the integer dot. `None` means the id is
/// missing or the row is consent-denied. The scan does not call `score`.
/// It calls [`Self::prepare_query`], [`Self::dot_allowed`], and
/// [`Self::lift`] so the query bytes stay in the caller's buffer.
///
/// `dot_allowed` does not read the consent bit. The scan checks
/// [`Self::allowed`] first and does not dot a denied row.
pub trait Int8Scorer: Send + Sync {
    fn score(&self, item_id: &str, query: &str) -> Option<i32>;

    fn row_count(&self) -> usize;

    fn width(&self) -> usize;

    fn allowed(&self, row: usize) -> bool;

    /// Encode `text` into `buf`. `buf.len()` is [`Self::width`].
    fn prepare_query(&self, text: &str, buf: &mut [i8]) -> PreparedQuery;

    /// `false` skips the dot loop. A blank diary query has norm `0`.
    fn ranks_query(&self, prepared: PreparedQuery) -> bool;

    fn dot_allowed(&self, row: usize, query: &[i8], level: SimdLevel) -> i32;

    /// Rank score. `None` drops the row after the dot. The expression does
    /// not depend on `level`.
    fn lift(&self, row: usize, dot: i32, prepared: PreparedQuery) -> Option<f32>;
}

/// One person-diary row. `values` is [`DIARY_DIM`] int8 lanes.
pub struct DiaryRow<'a> {
    pub item_id: &'a str,
    pub values: &'a [i8],
    pub scale: f32,
    pub norm: f32,
    pub allow: bool,
}

/// In-memory person-diary matrix. Built at setup. The scan borrows it.
pub struct DiaryArena {
    keys: Vec<i8>,
    scale: Vec<f32>,
    norm: Vec<f32>,
    allow: Vec<bool>,
    item_id: Vec<String>,
    index: HashMap<String, usize>,
}

impl DiaryArena {
    pub fn from_rows<'a, I>(rows: I) -> Result<Self, ScanError>
    where
        I: IntoIterator<Item = DiaryRow<'a>>,
    {
        let mut keys = Vec::new();
        let mut scale = Vec::new();
        let mut norm = Vec::new();
        let mut allow = Vec::new();
        let mut item_id = Vec::new();
        let mut index = HashMap::new();
        for row in rows {
            if row.item_id.is_empty() {
                return Err(ScanError::EmptyItemId);
            }
            if row.values.len() != DIARY_DIM {
                return Err(ScanError::Width {
                    got: row.values.len(),
                    expect: DIARY_DIM,
                });
            }
            if index
                .insert(row.item_id.to_string(), item_id.len())
                .is_some()
            {
                return Err(ScanError::DuplicateItemId);
            }
            item_id.push(row.item_id.to_string());
            keys.extend_from_slice(row.values);
            scale.push(row.scale);
            norm.push(row.norm);
            allow.push(row.allow);
        }
        Ok(Self {
            keys,
            scale,
            norm,
            allow,
            item_id,
            index,
        })
    }

    pub fn len(&self) -> usize {
        self.item_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.item_id.is_empty()
    }

    pub fn item_id(&self, row: usize) -> Option<&str> {
        self.item_id.get(row).map(String::as_str)
    }

    pub fn row(&self, index: usize) -> Option<&[i8]> {
        let start = index.checked_mul(DIARY_DIM)?;
        let end = start.checked_add(DIARY_DIM)?;
        self.keys.get(start..end)
    }

    pub fn row_scale(&self, row: usize) -> Option<f32> {
        self.scale.get(row).copied()
    }

    pub fn row_norm(&self, row: usize) -> Option<f32> {
        self.norm.get(row).copied()
    }
}

impl Int8Scorer for DiaryArena {
    fn score(&self, item_id: &str, query: &str) -> Option<i32> {
        let &row = self.index.get(item_id)?;
        if !self.allowed(row) {
            return None;
        }
        let mut encoded = [0i8; DIARY_DIM];
        self.prepare_query(query, &mut encoded);
        Some(self.dot_allowed(row, &encoded, features::selected()))
    }

    fn row_count(&self) -> usize {
        self.len()
    }

    fn width(&self) -> usize {
        DIARY_DIM
    }

    fn allowed(&self, row: usize) -> bool {
        self.allow.get(row).copied().unwrap_or(false)
    }

    fn prepare_query(&self, text: &str, buf: &mut [i8]) -> PreparedQuery {
        match embed_diary_into(text, buf) {
            Ok(embed) => PreparedQuery {
                scale: embed.scale,
                norm: embed.norm,
            },
            Err(_) => PreparedQuery {
                scale: 1.0,
                norm: 0.0,
            },
        }
    }

    fn ranks_query(&self, prepared: PreparedQuery) -> bool {
        prepared.norm != 0.0
    }

    fn dot_allowed(&self, row: usize, query: &[i8], level: SimdLevel) -> i32 {
        let Some(keys) = self.row(row) else {
            return 0;
        };
        if keys.len() != query.len() {
            return 0;
        }
        dot_i8_for(level)(keys, query)
    }

    fn lift(&self, row: usize, dot: i32, prepared: PreparedQuery) -> Option<f32> {
        let norm = self.row_norm(row).unwrap_or(0.0);
        let scale = self.row_scale(row).unwrap_or(0.0);
        if prepared.norm == 0.0 || norm == 0.0 || dot <= 0 {
            return None;
        }
        Some((dot as f32) * prepared.scale * scale / (prepared.norm * norm))
    }
}

impl Int8Scorer for StoredKeys {
    fn score(&self, item_id: &str, query: &str) -> Option<i32> {
        let row = self.row_of(item_id)?;
        if !self.allowed(row) {
            return None;
        }
        let mut encoded = vec![0i8; self.width()];
        let _ = self.prepare_query(query, &mut encoded);
        Some(self.dot_allowed(row, &encoded, features::selected()))
    }

    fn row_count(&self) -> usize {
        self.len()
    }

    fn width(&self) -> usize {
        self.key_dim()
    }

    fn allowed(&self, row: usize) -> bool {
        self.allows(row).unwrap_or(false)
    }

    fn prepare_query(&self, text: &str, buf: &mut [i8]) -> PreparedQuery {
        if buf.len() == self.width() {
            encode_query_into(text, buf);
        }
        PreparedQuery {
            scale: 1.0,
            norm: 1.0,
        }
    }

    fn ranks_query(&self, _prepared: PreparedQuery) -> bool {
        true
    }

    fn dot_allowed(&self, row: usize, query: &[i8], level: SimdLevel) -> i32 {
        let Ok(keys) = self.row(row) else {
            return 0;
        };
        if keys.len() != query.len() {
            return 0;
        }
        dot_i8_for(level)(keys, query)
    }

    fn lift(&self, row: usize, dot: i32, _prepared: PreparedQuery) -> Option<f32> {
        let Ok(scale) = self.scale(row) else {
            return None;
        };
        Some(dequant_score(dot, scale))
    }
}

/// Feature-hash `text` into `values` (`values.len() == `[`DIARY_DIM`]).
///
/// Same shape as the diary's hashing embedder: lowercase alphanumeric
/// tokens, two signed lanes, L2 norm, then symmetric int8 with one scale.
/// A token longer than 64 bytes is dropped. Stopwords stay in the vector.
/// The query scale is the value returned here. This function does not
/// allocate.
pub fn embed_diary_into(text: &str, values: &mut [i8]) -> Result<DiaryEmbed, ScanError> {
    if values.len() != DIARY_DIM {
        return Err(ScanError::Width {
            got: values.len(),
            expect: DIARY_DIM,
        });
    }
    let mut acc = [0f32; DIARY_DIM];
    hash_tokens(text, &mut acc);
    normalize(&mut acc);
    Ok(quantize_into(&acc, values))
}

/// `horbm_keys::encode_query_i8` written into `out`. The mixer has no scale.
/// This function does not allocate.
pub fn encode_query_into(text: &str, out: &mut [i8]) {
    let mut state = FNV_OFFSET;
    for byte in text.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    for (i, slot) in out.iter_mut().enumerate() {
        let mut x = state ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        *slot = x as u8 as i8;
    }
}

struct Arenas {
    keys: StoredKeys,
    diary: DiaryArena,
}

struct Scratch {
    key_query: Vec<i8>,
    diary_query: Vec<i8>,
    key_top: Vec<ScanHit>,
    diary_top: Vec<ScanHit>,
}

impl Scratch {
    fn new(key_width: usize, diary_width: usize, k: usize) -> Self {
        Self {
            key_query: vec![0; key_width],
            diary_query: vec![0; diary_width],
            key_top: vec![ScanHit::default(); k],
            diary_top: vec![ScanHit::default(); k],
        }
    }
}

/// Both arenas, the consent flag, and the scratch a query reuses.
pub struct DiaryScan {
    arenas: Arenas,
    scratch: Scratch,
    remember: bool,
    k: usize,
}

impl DiaryScan {
    /// `k` is the per-arena pool and the most hits one query returns.
    /// `remember` false matches `LOCKS.toml` `[consent] remember = "deny"`
    /// and leaves the diary out. Key rows still use their own allow bits.
    pub fn open(keys: StoredKeys, diary: DiaryArena, remember: bool, k: usize) -> Self {
        let scratch = Scratch::new(keys.key_dim(), DIARY_DIM, k);
        Self {
            arenas: Arenas { keys, diary },
            scratch,
            remember,
            k,
        }
    }

    /// Same as [`Self::open`] with `k` fixed at [`SCAN_CAP`].
    ///
    /// Setup for the retrieval thread. The cap is [`SCAN_CAP`], the same
    /// value as `horbm::MEMORY_HIT_CAP`, so that call cannot pick another.
    pub fn open_capped(keys: StoredKeys, diary: DiaryArena, remember: bool) -> Self {
        Self::open(keys, diary, remember, SCAN_CAP)
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn remember(&self) -> bool {
        self.remember
    }

    pub fn set_remember(&mut self, grant: bool) {
        self.remember = grant;
    }

    pub fn keys(&self) -> &StoredKeys {
        &self.arenas.keys
    }

    pub fn diary(&self) -> &DiaryArena {
        &self.arenas.diary
    }

    pub fn active_kernel() -> SimdLevel {
        StoredKeys::active_kernel()
    }

    /// Id stored for `hit`. `None` on a version-1 key row, which has no string.
    pub fn item_id(&self, hit: ScanHit) -> Option<&str> {
        match hit.pool {
            ScanPool::Diary => self.arenas.diary.item_id(hit.row as usize),
            ScanPool::Keys => self.arenas.keys.item_id(hit.row as usize).ok().flatten(),
        }
    }

    /// Selected kernel. Same bytes as [`Self::scan_scalar_into`].
    #[must_use]
    pub fn scan_selected_into(&mut self, query: &str, out: &mut [ScanHit]) -> usize {
        self.scan_at(features::selected(), query, out)
    }

    /// Scalar oracle.
    #[must_use]
    pub fn scan_scalar_into(&mut self, query: &str, out: &mut [ScanHit]) -> usize {
        self.scan_at(SimdLevel::Scalar, query, out)
    }

    /// `level` is a request. [`dot_i8_for`] applies the AVX-512BW rule.
    #[must_use]
    pub fn scan_at(&mut self, level: SimdLevel, query: &str, out: &mut [ScanHit]) -> usize {
        scan_arenas(
            &self.arenas.keys,
            &self.arenas.diary,
            self.remember,
            query,
            level,
            &mut self.scratch,
            out,
        )
    }
}

fn scan_arenas<H, D>(
    keys: &H,
    diary: &D,
    remember: bool,
    query: &str,
    level: SimdLevel,
    scratch: &mut Scratch,
    out: &mut [ScanHit],
) -> usize
where
    H: Int8Scorer,
    D: Int8Scorer,
{
    let limit = out
        .len()
        .min(scratch.key_top.len())
        .min(scratch.diary_top.len());
    if limit == 0 {
        return 0;
    }
    let key_n = rank_arena(
        keys,
        query,
        level,
        &mut scratch.key_query,
        ScanPool::Keys,
        &mut scratch.key_top[..limit],
    );
    let diary_n = if remember {
        rank_arena(
            diary,
            query,
            level,
            &mut scratch.diary_query,
            ScanPool::Diary,
            &mut scratch.diary_top[..limit],
        )
    } else {
        0
    };
    fuse(
        &scratch.diary_top[..diary_n],
        &scratch.key_top[..key_n],
        &mut out[..limit],
    )
}

fn rank_arena<S: Int8Scorer>(
    scorer: &S,
    query: &str,
    level: SimdLevel,
    buf: &mut [i8],
    pool: ScanPool,
    top: &mut [ScanHit],
) -> usize {
    if top.is_empty() || scorer.row_count() == 0 || buf.len() != scorer.width() {
        return 0;
    }
    let prepared = scorer.prepare_query(query, buf);
    if !scorer.ranks_query(prepared) {
        return 0;
    }
    let mut n = 0usize;
    for row in 0..scorer.row_count() {
        if !scorer.allowed(row) {
            continue;
        }
        let Ok(row_u) = u32::try_from(row) else {
            continue;
        };
        let dot = scorer.dot_allowed(row, buf, level);
        let Some(native) = scorer.lift(row, dot, prepared) else {
            continue;
        };
        consider(top, &mut n, pool, row_u, native);
    }
    sort_hits(&mut top[..n]);
    n
}

fn consider(top: &mut [ScanHit], n: &mut usize, pool: ScanPool, row: u32, native: f32) {
    let hit = ScanHit {
        pool,
        row,
        score: 0.0,
        native,
    };
    if *n < top.len() {
        top[*n] = hit;
        *n += 1;
        return;
    }
    let mut worst = 0usize;
    for i in 1..top.len() {
        if ranks_before(&top[worst], &top[i]) {
            worst = i;
        }
    }
    if ranks_before(&hit, &top[worst]) {
        top[worst] = hit;
    }
}

fn sort_hits(top: &mut [ScanHit]) {
    for i in 1..top.len() {
        let mut j = i;
        while j > 0 && ranks_before(&top[j], &top[j - 1]) {
            top.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// Higher native first. Equal natives keep the lower row.
fn ranks_before(a: &ScanHit, b: &ScanHit) -> bool {
    match a.native.total_cmp(&b.native) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => a.row < b.row,
    }
}

/// Top-k merge by reciprocal rank. Each list is already best-first.
/// Rank `r` scores `1 / (SCAN_RRF_K + r)`. The same rank from both arenas
/// ties; the diary hit is written first, then the key hit.
fn fuse(diary: &[ScanHit], keys: &[ScanHit], out: &mut [ScanHit]) -> usize {
    let mut written = 0usize;
    let mut rank = 1u32;
    while written < out.len() {
        let diary_ok = (rank as usize) <= diary.len();
        let keys_ok = (rank as usize) <= keys.len();
        if !diary_ok && !keys_ok {
            break;
        }
        if diary_ok {
            out[written] = stamp(&diary[rank as usize - 1], rank);
            written += 1;
            if written == out.len() {
                break;
            }
        }
        if keys_ok {
            out[written] = stamp(&keys[rank as usize - 1], rank);
            written += 1;
        }
        rank += 1;
    }
    written
}

fn stamp(hit: &ScanHit, rank: u32) -> ScanHit {
    ScanHit {
        pool: hit.pool,
        row: hit.row,
        native: hit.native,
        score: 1.0 / (SCAN_RRF_K + rank as f32),
    }
}

fn hash_tokens(text: &str, acc: &mut [f32]) {
    let mut buf = [0u8; TOKEN_MAX];
    let mut len = 0usize;
    let mut overflow = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                let mut tmp = [0u8; 4];
                let encoded = lower.encode_utf8(&mut tmp);
                if overflow || len + encoded.len() > TOKEN_MAX {
                    overflow = true;
                    continue;
                }
                buf[len..len + encoded.len()].copy_from_slice(encoded.as_bytes());
                len += encoded.len();
            }
        } else {
            flush_token(&buf, &mut len, &mut overflow, acc);
        }
    }
    flush_token(&buf, &mut len, &mut overflow, acc);
}

fn flush_token(buf: &[u8], len: &mut usize, overflow: &mut bool, acc: &mut [f32]) {
    if !*overflow && *len > 0 {
        add_token(&buf[..*len], acc);
    }
    *len = 0;
    *overflow = false;
}

fn add_token(token: &[u8], acc: &mut [f32]) {
    let h = fnv1a(token);
    let i1 = (h as usize) % DIARY_DIM;
    let i2 = ((h >> 8) as usize) % DIARY_DIM;
    let s1 = if h & 1 == 0 { 1.0 } else { -1.0 };
    let s2 = if (h >> 1) & 1 == 0 { 1.0 } else { -1.0 };
    acc[i1] += s1;
    if i2 != i1 {
        acc[i2] += s2 * 0.5;
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for byte in bytes {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn normalize(acc: &mut [f32]) {
    let mut norm_sq = 0f32;
    for value in acc.iter() {
        norm_sq += value * value;
    }
    let n = norm_sq.sqrt();
    if n > 0.0 {
        for value in acc.iter_mut() {
            *value /= n;
        }
    }
}

fn quantize_into(acc: &[f32], values: &mut [i8]) -> DiaryEmbed {
    let mut max = 0f32;
    for value in acc {
        max = max.max(value.abs());
    }
    let scale = if max == 0.0 { 1.0 } else { max / 127.0 };
    let mut norm_sq = 0f32;
    for (slot, value) in values.iter_mut().zip(acc) {
        let q = (value / scale).round().clamp(-127.0, 127.0) as i8;
        *slot = q;
        let recon = f32::from(q) * scale;
        norm_sq += recon * recon;
    }
    DiaryEmbed {
        scale,
        norm: norm_sq.sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use galion_tensor::features;
    use horbm_keys::{encode_query_i8, KeyPack};

    use super::*;
    use crate::StoredPack;

    fn bits(score: f32) -> u32 {
        score.to_bits()
    }

    fn rrf(rank: u32) -> f32 {
        1.0 / (SCAN_RRF_K + rank as f32)
    }

    fn reference_dot(row: &[i8], query: &[i8]) -> i32 {
        let mut acc = 0i32;
        for (&a, &b) in row.iter().zip(query) {
            acc = acc.wrapping_add(i32::from(a) * i32::from(b));
        }
        acc
    }

    fn reference_embed(text: &str) -> (Vec<i8>, DiaryEmbed) {
        let mut tokens = Vec::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() {
                for lower in ch.to_lowercase() {
                    cur.push(lower);
                }
            } else if !cur.is_empty() {
                push_token(&mut tokens, &mut cur);
            }
        }
        if !cur.is_empty() {
            push_token(&mut tokens, &mut cur);
        }
        let mut acc = vec![0f32; DIARY_DIM];
        for token in &tokens {
            let mut h = FNV_OFFSET;
            for byte in token.as_bytes() {
                h ^= u64::from(*byte);
                h = h.wrapping_mul(FNV_PRIME);
            }
            let i1 = (h as usize) % DIARY_DIM;
            let i2 = ((h >> 8) as usize) % DIARY_DIM;
            let s1 = if h & 1 == 0 { 1.0 } else { -1.0 };
            let s2 = if (h >> 1) & 1 == 0 { 1.0 } else { -1.0 };
            acc[i1] += s1;
            if i2 != i1 {
                acc[i2] += s2 * 0.5;
            }
        }
        let mut norm_sq = 0f32;
        for value in &acc {
            norm_sq += value * value;
        }
        let n = norm_sq.sqrt();
        if n > 0.0 {
            for value in &mut acc {
                *value /= n;
            }
        }
        let mut max = 0f32;
        for value in &acc {
            max = max.max(value.abs());
        }
        let scale = if max == 0.0 { 1.0 } else { max / 127.0 };
        let mut values = vec![0i8; DIARY_DIM];
        let mut recon_sq = 0f32;
        for (slot, value) in values.iter_mut().zip(&acc) {
            let q = (value / scale).round().clamp(-127.0, 127.0) as i8;
            *slot = q;
            let recon = f32::from(q) * scale;
            recon_sq += recon * recon;
        }
        (
            values,
            DiaryEmbed {
                scale,
                norm: recon_sq.sqrt(),
            },
        )
    }

    fn push_token(out: &mut Vec<String>, cur: &mut String) {
        if cur.len() <= TOKEN_MAX {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    }

    fn sample_keys() -> StoredKeys {
        let k = 8usize;
        let n = 5usize;
        let mut keys = vec![0i8; n * k];
        for row in 0..n {
            for lane in 0..k {
                keys[row * k + lane] = (row as i8) * 3 - (lane as i8);
            }
        }
        let pack = StoredPack {
            k,
            keys,
            clause_id: vec![10, 11, 12, 13, 14],
            allow: vec![false, true, true, true, true],
            source: vec![1; n],
            item_id: vec![
                "k0".into(),
                "k1".into(),
                "k2".into(),
                "k3".into(),
                "k4".into(),
            ],
            scale: vec![1, -3, 2, 0, 5],
        };
        StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap()
    }

    fn diary_from(spec: &[(&str, &str, bool)]) -> DiaryArena {
        let mut owned = Vec::new();
        for (id, text, allow) in spec {
            let mut values = vec![0i8; DIARY_DIM];
            let embed = embed_diary_into(text, &mut values).unwrap();
            owned.push((*id, values, embed, *allow));
        }
        DiaryArena::from_rows(owned.iter().map(|(id, values, embed, allow)| DiaryRow {
            item_id: id,
            values,
            scale: embed.scale,
            norm: embed.norm,
            allow: *allow,
        }))
        .unwrap()
    }

    fn sample_diary() -> DiaryArena {
        diary_from(&[
            ("d-deny", "alpha token shared", false),
            ("d-near", "alpha token shared", true),
            ("d-far", "piper synthesizes a short clause", true),
            ("d-mid", "alpha token of a different claim", true),
        ])
    }

    fn open_sample(remember: bool, k: usize) -> DiaryScan {
        DiaryScan::open(sample_keys(), sample_diary(), remember, k)
    }

    fn fill(scan: &mut DiaryScan, level: SimdLevel, query: &str) -> Vec<ScanHit> {
        let mut out = vec![ScanHit::default(); scan.k()];
        let n = scan.scan_at(level, query, &mut out);
        out.truncate(n);
        out
    }

    fn same(a: &[ScanHit], b: &[ScanHit]) {
        assert_eq!(a.len(), b.len());
        for (i, (left, right)) in a.iter().zip(b).enumerate() {
            assert_eq!(left.pool, right.pool, "pool {i}");
            assert_eq!(left.row, right.row, "row {i}");
            assert_eq!(bits(left.score), bits(right.score), "score {i}");
            assert_eq!(bits(left.native), bits(right.native), "native {i}");
        }
    }

    struct Prog {
        allow: Vec<bool>,
        native: Vec<f32>,
        width: usize,
        query_norm: f32,
        dots: AtomicUsize,
        prepares: AtomicUsize,
    }

    impl Prog {
        fn new(allow: Vec<bool>, native: Vec<f32>) -> Self {
            Self {
                allow,
                native,
                width: 1,
                query_norm: 1.0,
                dots: AtomicUsize::new(0),
                prepares: AtomicUsize::new(0),
            }
        }
    }

    impl Int8Scorer for Prog {
        fn score(&self, item_id: &str, _query: &str) -> Option<i32> {
            let row: usize = item_id.parse().ok()?;
            if !self.allowed(row) {
                return None;
            }
            Some(row as i32)
        }

        fn row_count(&self) -> usize {
            self.native.len()
        }

        fn width(&self) -> usize {
            self.width
        }

        fn allowed(&self, row: usize) -> bool {
            self.allow.get(row).copied().unwrap_or(false)
        }

        fn prepare_query(&self, _text: &str, buf: &mut [i8]) -> PreparedQuery {
            self.prepares.fetch_add(1, Ordering::SeqCst);
            if let Some(slot) = buf.first_mut() {
                *slot = 1;
            }
            PreparedQuery {
                scale: 1.0,
                norm: self.query_norm,
            }
        }

        fn ranks_query(&self, prepared: PreparedQuery) -> bool {
            prepared.norm != 0.0
        }

        fn dot_allowed(&self, _row: usize, _query: &[i8], _level: SimdLevel) -> i32 {
            self.dots.fetch_add(1, Ordering::SeqCst);
            1
        }

        fn lift(&self, row: usize, _dot: i32, _prepared: PreparedQuery) -> Option<f32> {
            self.native.get(row).copied()
        }
    }

    #[test]
    fn open_capped_uses_scan_cap() {
        let mut capped = DiaryScan::open_capped(sample_keys(), sample_diary(), true);
        assert_eq!(capped.k(), SCAN_CAP);
        assert!(capped.remember());

        let mut opened = DiaryScan::open(sample_keys(), sample_diary(), true, SCAN_CAP);
        assert_eq!(opened.k(), capped.k());

        let query = "alpha token shared";
        let mut wide = [ScanHit::default(); SCAN_CAP + 4];
        let n = capped.scan_selected_into(query, &mut wide);
        assert!(n > 0);
        assert!(n <= SCAN_CAP);
        let mut expect = [ScanHit::default(); SCAN_CAP + 4];
        let m = opened.scan_selected_into(query, &mut expect);
        same(&wide[..n], &expect[..m]);
        assert!(wide[SCAN_CAP..]
            .iter()
            .all(|hit| *hit == ScanHit::default()));

        let denied = DiaryScan::open_capped(sample_keys(), sample_diary(), false);
        assert_eq!(denied.k(), SCAN_CAP);
        assert!(!denied.remember());
    }

    #[test]
    fn the_query_mixer_matches_horbm_keys_and_the_diary_hash_matches_its_reference() {
        for text in ["", "q-lane", "where is the kettle", "id-β", "İ"] {
            for k in [1usize, 7, 32, 256] {
                let mut buf = vec![0i8; k];
                encode_query_into(text, &mut buf);
                assert_eq!(buf, encode_query_i8(text, k), "{text} k {k}");
            }
        }
        let long = format!("{} zz", "b".repeat(65));
        for text in [
            "",
            "...",
            "Ab",
            "the kettle is on the hob",
            "id-β",
            "İ",
            "a",
            &"c".repeat(64),
            long.as_str(),
        ] {
            let mut got = [0i8; DIARY_DIM];
            let embed = embed_diary_into(text, &mut got).unwrap();
            let (expect, meta) = reference_embed(text);
            assert_eq!(got.as_slice(), expect.as_slice(), "{text}");
            assert_eq!(bits(embed.scale), bits(meta.scale), "{text} scale");
            assert_eq!(bits(embed.norm), bits(meta.norm), "{text} norm");
        }
        assert!(matches!(
            embed_diary_into("ab", &mut [0i8; 4]),
            Err(ScanError::Width {
                got: 4,
                expect: DIARY_DIM
            })
        ));
    }

    #[test]
    fn identical_diary_text_beats_an_unrelated_line() {
        let mut query = [0i8; DIARY_DIM];
        let q = embed_diary_into("simhash neighbourhood merge", &mut query).unwrap();
        let mut near_v = [0i8; DIARY_DIM];
        let near =
            embed_diary_into("a simhash neighbourhood of consented claims", &mut near_v).unwrap();
        let mut far_v = [0i8; DIARY_DIM];
        let far =
            embed_diary_into("piper synthesizes a short clause of audio", &mut far_v).unwrap();
        let near_dot = reference_dot(&query, &near_v);
        let far_dot = reference_dot(&query, &far_v);
        let near_s = (near_dot as f32) * q.scale * near.scale / (q.norm * near.norm);
        let far_s = (far_dot as f32) * q.scale * far.scale / (q.norm * far.norm);
        assert!(near_s > far_s, "near {near_s} far {far_s}");
        let mut same_v = [0i8; DIARY_DIM];
        let same = embed_diary_into("the great wall at beijing", &mut same_v).unwrap();
        let mut wall = [0i8; DIARY_DIM];
        let wall_q = embed_diary_into("the great wall at beijing", &mut wall).unwrap();
        let dot = reference_dot(&wall, &same_v);
        let score = (dot as f32) * wall_q.scale * same.scale / (wall_q.norm * same.norm);
        assert!(score > 0.98, "score {score}");
    }

    #[test]
    fn the_s51_scale_example_is_the_key_lift_on_every_kernel() {
        let pack = StoredPack {
            k: 3,
            keys: vec![4, 1, -2],
            clause_id: vec![9],
            allow: vec![true],
            source: vec![4],
            item_id: vec!["row-0".into()],
            scale: vec![-3],
        };
        let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
        let query = [2i8, -5, 7];
        let mut padded = [0i8; 6];
        padded[1..4].copy_from_slice(&query);
        let unaligned = &padded[1..4];
        for level in [
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Avx512,
            SimdLevel::Neon,
        ] {
            for lanes in [&query[..], unaligned] {
                let dot = Int8Scorer::dot_allowed(&keys, 0, lanes, level);
                assert_eq!(dot, -11, "{level:?}");
                let lifted = Int8Scorer::lift(
                    &keys,
                    0,
                    dot,
                    PreparedQuery {
                        scale: 1.0,
                        norm: 1.0,
                    },
                )
                .unwrap();
                assert_eq!(bits(lifted), bits(33.0), "{level:?}");
            }
        }
        assert_eq!(
            Int8Scorer::score(&keys, "row-0", "q-lane"),
            Some(reference_dot(
                keys.row(0).unwrap(),
                &encode_query_i8("q-lane", 3)
            ))
        );
        assert_eq!(Int8Scorer::score(&keys, "missing", "q-lane"), None);
    }

    #[test]
    fn a_denied_row_is_absent_and_scalar_matches_every_kernel() {
        let query = "alpha token shared";
        let mut scan = open_sample(true, SCAN_CAP);
        let scalar = fill(&mut scan, SimdLevel::Scalar, query);
        assert!(scalar.iter().any(|hit| hit.pool == ScanPool::Diary));
        assert!(scalar.iter().any(|hit| hit.pool == ScanPool::Keys));
        assert!(scalar
            .iter()
            .all(|hit| scan.item_id(*hit) != Some("d-deny")));
        assert!(scalar.iter().all(|hit| scan.item_id(*hit) != Some("k0")));
        assert_eq!(scalar[0].pool, ScanPool::Diary);
        assert_eq!(scan.item_id(scalar[0]), Some("d-near"));
        assert_eq!(bits(scalar[0].score), bits(rrf(1)));

        let mut encoded = vec![0i8; scan.keys().key_dim()];
        encode_query_into(query, &mut encoded);
        for hit in scalar.iter().filter(|hit| hit.pool == ScanPool::Keys) {
            let row = hit.row as usize;
            let dot = reference_dot(scan.keys().row(row).unwrap(), &encoded);
            let expect = dequant_score(dot, scan.keys().scale(row).unwrap());
            assert_eq!(bits(hit.native), bits(expect), "key {row}");
            assert!(scan.keys().allows(row).unwrap());
        }
        let mut q = [0i8; DIARY_DIM];
        let qmeta = embed_diary_into(query, &mut q).unwrap();
        for hit in scalar.iter().filter(|hit| hit.pool == ScanPool::Diary) {
            let row = hit.row as usize;
            let dot = reference_dot(scan.diary().row(row).unwrap(), &q);
            let expect = (dot as f32) * qmeta.scale * scan.diary().row_scale(row).unwrap()
                / (qmeta.norm * scan.diary().row_norm(row).unwrap());
            assert_eq!(bits(hit.native), bits(expect), "diary {row}");
            assert!(scan.diary().allow[row]);
        }

        for level in [
            SimdLevel::Avx2,
            SimdLevel::Avx512,
            SimdLevel::Neon,
            features::selected(),
        ] {
            same(&scalar, &fill(&mut scan, level, query));
        }
        let selected = fill(&mut scan, features::selected(), query);
        assert_eq!(
            Int8Scorer::score(scan.keys(), "k1", query),
            Some(reference_dot(
                scan.keys().row(1).unwrap(),
                &encode_query_i8(query, scan.keys().key_dim())
            ))
        );
        assert_eq!(Int8Scorer::score(scan.keys(), "k0", query), None);
        assert_eq!(Int8Scorer::score(scan.diary(), "d-deny", query), None);
        let diary_dot = Int8Scorer::score(scan.diary(), "d-near", query).unwrap();
        assert_eq!(diary_dot, reference_dot(scan.diary().row(1).unwrap(), &q));
        assert!(selected
            .iter()
            .any(|hit| hit.row == 1 && hit.pool == ScanPool::Diary));
    }

    #[test]
    fn remember_deny_drops_the_diary_and_keeps_allowed_keys() {
        let query = "alpha token shared";
        let mut granted = open_sample(true, SCAN_CAP);
        let with_diary = fill(&mut granted, SimdLevel::Scalar, query);
        assert!(with_diary.iter().any(|hit| hit.pool == ScanPool::Diary));

        granted.set_remember(false);
        assert!(!granted.remember());
        let denied = fill(&mut granted, SimdLevel::Scalar, query);
        assert!(denied.iter().all(|hit| hit.pool == ScanPool::Keys));
        assert!(!denied.is_empty());
        for (i, hit) in denied.iter().enumerate() {
            assert_eq!(bits(hit.score), bits(rrf((i as u32) + 1)), "rank {}", i + 1);
        }
        let mut keys_only = open_sample(false, 1);
        let top = fill(&mut keys_only, SimdLevel::Scalar, query);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].pool, ScanPool::Keys);
        let best = denied
            .iter()
            .max_by(|a, b| a.native.total_cmp(&b.native).then(b.row.cmp(&a.row)))
            .unwrap();
        assert_eq!(top[0].row, best.row);
        assert_eq!(bits(top[0].native), bits(best.native));
    }

    #[test]
    fn a_one_slot_merge_keeps_the_diary_rank() {
        let mut scan = open_sample(true, 1);
        let hits = fill(&mut scan, SimdLevel::Scalar, "alpha token shared");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pool, ScanPool::Diary);
        assert_eq!(scan.item_id(hits[0]), Some("d-near"));
        assert_eq!(bits(hits[0].score), bits(rrf(1)));
    }

    #[test]
    fn a_blank_query_skips_the_diary_and_a_version_1_file_has_no_string_id() {
        let mut scan = open_sample(true, SCAN_CAP);
        let blank = fill(&mut scan, SimdLevel::Scalar, "...");
        assert!(blank.iter().all(|hit| hit.pool == ScanPool::Keys));
        assert!(!blank.is_empty());

        let pack = KeyPack {
            k: 4,
            keys: vec![1, 2, 3, 4, 5, 6, 7, 8],
            clause_id: vec![1, 2],
            allow: vec![false, true],
            source: vec![8, 9],
        };
        let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
        assert_eq!(keys.version(), 1);
        assert_eq!(keys.row_of("1"), None);
        assert_eq!(Int8Scorer::score(&keys, "1", "q"), None);
        let mut scan = DiaryScan::open(keys, DiaryArena::from_rows([]).unwrap(), true, 4);
        let hits = fill(&mut scan, SimdLevel::Scalar, "q");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pool, ScanPool::Keys);
        assert_eq!(hits[0].row, 1);
        assert!(scan.item_id(hits[0]).is_none());
        assert!(hits.iter().all(|hit| hit.row != 0));
    }

    #[test]
    fn diary_rows_reject_an_empty_id_a_duplicate_and_the_wrong_width() {
        let zeros = vec![0i8; DIARY_DIM];
        let bad_width = DiaryArena::from_rows([DiaryRow {
            item_id: "a",
            values: &[1, 2],
            scale: 1.0,
            norm: 1.0,
            allow: true,
        }]);
        assert!(matches!(
            bad_width,
            Err(ScanError::Width {
                got: 2,
                expect: DIARY_DIM
            })
        ));
        let empty = DiaryArena::from_rows([DiaryRow {
            item_id: "",
            values: &zeros,
            scale: 1.0,
            norm: 1.0,
            allow: true,
        }]);
        assert!(matches!(empty, Err(ScanError::EmptyItemId)));
        let dup = DiaryArena::from_rows([
            DiaryRow {
                item_id: "same",
                values: &zeros,
                scale: 1.0,
                norm: 1.0,
                allow: true,
            },
            DiaryRow {
                item_id: "same",
                values: &zeros,
                scale: 1.0,
                norm: 1.0,
                allow: false,
            },
        ]);
        assert!(matches!(dup, Err(ScanError::DuplicateItemId)));
        assert!(DiaryArena::from_rows([]).unwrap().is_empty());
    }

    #[test]
    fn the_scan_dots_only_allowed_rows_and_skips_a_denied_diary() {
        let keys = Prog::new(vec![false, true, true, true], vec![10.0, 3.0, 7.0, 7.0]);
        let mut diary = Prog::new(vec![true, true], vec![0.5, 0.9]);
        let mut scratch = Scratch::new(1, 1, 4);
        let mut out = [ScanHit::default(); 4];
        let n = scan_arenas(
            &keys,
            &diary,
            true,
            "q",
            SimdLevel::Scalar,
            &mut scratch,
            &mut out,
        );
        assert_eq!(n, 4);
        assert_eq!(keys.dots.load(Ordering::SeqCst), 3);
        assert_eq!(diary.dots.load(Ordering::SeqCst), 2);
        assert_eq!(out[0].pool, ScanPool::Diary);
        assert_eq!(out[0].row, 1);
        assert_eq!(out[1].pool, ScanPool::Keys);
        assert_eq!(out[1].row, 2);
        assert_eq!(out[2].pool, ScanPool::Diary);
        assert_eq!(out[2].row, 0);
        assert_eq!(out[3].pool, ScanPool::Keys);
        assert_eq!(out[3].row, 3);
        assert_eq!(bits(out[0].score), bits(rrf(1)));
        assert_eq!(bits(out[1].score), bits(rrf(1)));
        assert_eq!(bits(out[2].score), bits(rrf(2)));
        assert_eq!(bits(out[3].score), bits(rrf(2)));
        assert_eq!(bits(out[0].native), bits(0.9));
        assert_eq!(bits(out[3].native), bits(7.0));

        keys.dots.store(0, Ordering::SeqCst);
        diary.dots.store(0, Ordering::SeqCst);
        diary.prepares.store(0, Ordering::SeqCst);
        let n = scan_arenas(
            &keys,
            &diary,
            false,
            "q",
            SimdLevel::Scalar,
            &mut scratch,
            &mut out,
        );
        assert_eq!(n, 3);
        assert_eq!(diary.prepares.load(Ordering::SeqCst), 0);
        assert_eq!(diary.dots.load(Ordering::SeqCst), 0);
        assert_eq!(keys.dots.load(Ordering::SeqCst), 3);
        assert!(out[..n].iter().all(|hit| hit.pool == ScanPool::Keys));

        diary.query_norm = 0.0;
        diary.dots.store(0, Ordering::SeqCst);
        let n = scan_arenas(
            &keys,
            &diary,
            true,
            "...",
            SimdLevel::Scalar,
            &mut scratch,
            &mut out,
        );
        assert!(out[..n].iter().all(|hit| hit.pool == ScanPool::Keys));
        assert_eq!(diary.dots.load(Ordering::SeqCst), 0);
        assert!(diary.prepares.load(Ordering::SeqCst) >= 1);

        let n = scan_arenas(
            &keys,
            &diary,
            true,
            "q",
            SimdLevel::Scalar,
            &mut scratch,
            &mut [],
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn a_zero_norm_diary_row_is_not_a_hit() {
        let query = "alpha token shared";
        let mut live = [0i8; DIARY_DIM];
        let embed = embed_diary_into(query, &mut live).unwrap();
        let zeros = [0i8; DIARY_DIM];
        let arena = DiaryArena::from_rows([
            DiaryRow {
                item_id: "zero",
                values: &zeros,
                scale: 1.0,
                norm: 0.0,
                allow: true,
            },
            DiaryRow {
                item_id: "live",
                values: &live,
                scale: embed.scale,
                norm: embed.norm,
                allow: true,
            },
        ])
        .unwrap();
        let mut scan = DiaryScan::open(sample_keys(), arena, true, 4);
        let hits = fill(&mut scan, SimdLevel::Scalar, query);
        assert!(hits.iter().all(|hit| scan.item_id(*hit) != Some("zero")));
        assert!(hits.iter().any(|hit| scan.item_id(*hit) == Some("live")));
    }

    /// Not a gate. `cargo test -p horbm-brain -- --ignored --nocapture timed_scan`.
    #[test]
    #[ignore = "micro-benchmark, not a gate"]
    fn timed_scan_of_one_thousand_rows() {
        const N: usize = 1000;
        const KEY_DIM: usize = 32;
        const WARM: usize = 10;
        const SAMPLES: usize = 100;
        let mut key_bytes = vec![0i8; N * KEY_DIM];
        for (i, lane) in key_bytes.iter_mut().enumerate() {
            *lane = (i as i8).wrapping_mul(3).wrapping_add(1);
        }
        let pack = StoredPack {
            k: KEY_DIM,
            keys: key_bytes,
            clause_id: (0..N as u64).collect(),
            allow: (0..N).map(|i| i % 17 != 0).collect(),
            source: vec![1; N],
            item_id: (0..N).map(|i| format!("k{i}")).collect(),
            scale: (0..N).map(|i| ((i % 7) as i8) - 3).collect(),
        };
        let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
        let mut rows = Vec::with_capacity(N);
        for i in 0..N {
            let mut values = vec![0i8; DIARY_DIM];
            for (lane, slot) in values.iter_mut().enumerate() {
                *slot = ((i + lane) as i8).wrapping_mul(5).wrapping_add(2);
            }
            rows.push((format!("d{i}"), values, i % 19 != 0));
        }
        let diary = DiaryArena::from_rows(rows.iter().map(|(id, values, allow)| DiaryRow {
            item_id: id,
            values,
            scale: 0.02,
            norm: 1.0,
            allow: *allow,
        }))
        .unwrap();
        let mut scan = DiaryScan::open(keys, diary, true, SCAN_CAP);
        let query = "where is the kettle";
        let mut out = [ScanHit::default(); SCAN_CAP];
        for _ in 0..WARM {
            let n = scan.scan_selected_into(query, &mut out);
            assert!(n > 0);
        }
        let mut samples = [0u64; SAMPLES];
        for sample in &mut samples {
            let start = std::time::Instant::now();
            let n = scan.scan_selected_into(query, &mut out);
            *sample = start.elapsed().as_nanos() as u64;
            assert!(n > 0);
        }
        samples.sort_unstable();
        println!(
            "s54 scan min {} ns median {} ns over {SAMPLES} queries, {N} keys k={KEY_DIM}, {N} diary k={DIARY_DIM}, kernel {}",
            samples[0],
            samples[SAMPLES / 2],
            DiaryScan::active_kernel().name()
        );
    }
}
