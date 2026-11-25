// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! String item ids over an [`super::KeyArena`].
//!
//! The file format stays `HKB1` with `u64` clause ids. The caller binds one
//! string per row. A missing id and a consent-denied row both return `None`.
//! The dot itself is the arena's scalar or selected kernel, so a selected
//! score is the scalar score on the same encoded query.

use std::collections::HashMap;

use galion_tensor::features::{self, SimdLevel};
use galion_tensor::gq::Prefault;

use super::{ArenaError, KeyArena};

/// FNV-1a over the query bytes, then one mix per lane. Portable. This is not
/// the diary's hashing embedder and it does not read a card.
pub fn encode_query_i8(text: &str, k: usize) -> Vec<i8> {
    let mut state = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x100000001b3);
    }
    let mut out = Vec::with_capacity(k);
    for i in 0..k {
        let mut x = state ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        out.push(x as u8 as i8);
    }
    out
}

/// Mapped arena plus the string keys [`crate::Int8Scorer`] would use.
///
/// `horbm-keys` does not depend on the `horbm` crate. The trait impl lives
/// next to the trait. This type is the arena side of that impl.
pub struct StringKeyArena {
    arena: KeyArena,
    index: HashMap<String, usize>,
}

impl StringKeyArena {
    pub fn open(
        path: impl AsRef<std::path::Path>,
        pf: Prefault,
        item_ids: &[String],
    ) -> Result<Self, ArenaError> {
        Self::bind(KeyArena::open(path, pf)?, item_ids.iter().map(String::as_str))
    }

    pub fn bind<I, S>(arena: KeyArena, item_ids: I) -> Result<Self, ArenaError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut index = HashMap::new();
        for (row, id) in item_ids.into_iter().enumerate() {
            let id = id.as_ref();
            if id.is_empty() {
                return Err(ArenaError::EmptyItemId);
            }
            if row >= arena.len() {
                return Err(ArenaError::LenMismatch {
                    field: "item_id",
                    got: row + 1,
                    expect: arena.len(),
                });
            }
            if index.insert(id.to_string(), row).is_some() {
                return Err(ArenaError::DuplicateItemId);
            }
        }
        if index.len() != arena.len() {
            return Err(ArenaError::LenMismatch {
                field: "item_id",
                got: index.len(),
                expect: arena.len(),
            });
        }
        Ok(Self { arena, index })
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Scalar oracle. `None` when `item_id` is absent or the row is denied.
    pub fn score_scalar_text(&self, item_id: &str, query: &str) -> Option<i32> {
        self.score_at(item_id, query, SimdLevel::Scalar)
    }

    /// Same bytes as [`Self::score_scalar_text`], through the selected kernel.
    pub fn score_selected_text(&self, item_id: &str, query: &str) -> Option<i32> {
        self.score_at(item_id, query, features::selected())
    }

    fn score_at(&self, item_id: &str, query: &str, level: SimdLevel) -> Option<i32> {
        let &row = self.index.get(item_id)?;
        if !self.arena.allows(row).ok()? {
            return None;
        }
        let encoded = encode_query_i8(query, self.arena.key_dim());
        let mut out = vec![0i32; self.arena.len()];
        self.arena.dots_into(&encoded, level, &mut out).ok()?;
        Some(out[row])
    }
}
