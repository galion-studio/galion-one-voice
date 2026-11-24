// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::collections::HashMap;

/// Reciprocal rank fusion. `k` is the Cormack, Clarke, and Büttcher constant.
/// Ranks are 1-based positions in each list. A higher score is a better hit.
pub fn fuse(lists: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut scores: HashMap<String, f32> = HashMap::new();
    for list in lists {
        for (idx, id) in list.iter().enumerate() {
            let rank = (idx + 1) as f32;
            let entry = scores.entry(id.clone()).or_insert(0.0);
            *entry += 1.0 / (k + rank);
        }
    }
    let mut ranked: Vec<(String, f32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_beats_a_single_list() {
        let lexical = vec!["a".into(), "b".into()];
        let dense = vec!["b".into(), "c".into()];
        let fused = fuse(&[lexical, dense], 60.0);
        assert_eq!(fused[0].0, "b");
        assert!(fused[0].1 > fused[1].1);
    }

    #[test]
    fn empty_lists() {
        assert!(fuse(&[Vec::new(), Vec::new()], 60.0).is_empty());
    }
}
