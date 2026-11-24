// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! [`crate::Int8Scorer`] for the mapped int8 arena.
//!
//! The retrieval thread does not call this. Asks still use `search_lexical`.
//! `None` means the arena has no allowed row for that string `item_id`.

use horbm_keys::StringKeyArena;

use crate::Int8Scorer;

impl Int8Scorer for StringKeyArena {
    fn score(&self, item_id: &str, query: &str) -> Option<i32> {
        self.score_selected_text(item_id, query)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use horbm_keys::{encode_query_i8, fixed_fixture, KeyPrefault, StringKeyArena};

    use crate::Int8Scorer;

    #[test]
    fn int8_scorer_is_bit_exact_with_scalar_on_the_fixture() {
        let (pack, _) = fixed_fixture();
        let ids: Vec<String> = (0..pack.n()).map(|i| format!("item-{i}")).collect();
        let dir = std::env::temp_dir().join(format!(
            "horbm-int8-trait-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.hkb");
        pack.write(&path).unwrap();
        let arena = StringKeyArena::open(&path, KeyPrefault::Populate, &ids).unwrap();
        let text = "fixture-query";
        let query = encode_query_i8(text, pack.k);
        for i in 0..pack.n() {
            let id = format!("item-{i}");
            let via_trait = Int8Scorer::score(&arena, &id, text);
            let scalar = arena.score_scalar_text(&id, text);
            assert_eq!(via_trait, scalar, "row {i}");
            if pack.allow[i] {
                let row = &pack.keys[i * pack.k..(i + 1) * pack.k];
                let acc: i64 = row
                    .iter()
                    .zip(query.iter())
                    .map(|(&a, &b)| i64::from(a) * i64::from(b))
                    .sum();
                assert_eq!(via_trait, Some(acc as i32), "row {i}");
            } else {
                assert_eq!(via_trait, None, "denied row {i}");
            }
        }
        assert_eq!(Int8Scorer::score(&arena, "sample-742", text), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
