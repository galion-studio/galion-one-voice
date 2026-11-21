// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Nearest-rank percentile with a ceiling index.
//!
//! Same index as `rank` in `crates/galion-heartbeat/tests/frame_budget.rs`:
//! `ceil(n * numer / denom) - 1`, clamped to the last sample. The slice must
//! already be sorted ascending. For `n < 100`, p99 is the maximum. The
//! report states `n` beside every percentile so that is visible.

/// `ceil(n * numer / denom) - 1`, clamped to `n - 1`.
pub fn rank(n: usize, numer: usize, denom: usize) -> usize {
    assert!(n > 0, "rank of an empty sample");
    assert!(denom > 0, "rank denominator");
    let ceil = n.saturating_mul(numer).saturating_add(denom - 1) / denom;
    ceil.saturating_sub(1).min(n - 1)
}

/// Percentile of a sorted ascending slice. `numer` is 50, 95, or 99 and
/// `denom` is 100 for the published reports.
pub fn percentile(sorted_asc: &[u64], numer: usize, denom: usize) -> u64 {
    sorted_asc[rank(sorted_asc.len(), numer, denom)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_heartbeat_rank_examples() {
        // frame_budget uses N=256. p99 index is 253, not the max.
        assert_eq!(rank(256, 50, 100), 127);
        assert_eq!(rank(256, 95, 100), 243);
        assert_eq!(rank(256, 99, 100), 253);
        assert_eq!(rank(1, 99, 100), 0);
        // n < 100: p99 is the last sample.
        assert_eq!(rank(32, 99, 100), 31);
        assert_eq!(rank(100, 99, 100), 98);
    }

    #[test]
    fn percentile_reads_that_index() {
        let s = [10, 20, 30, 40, 100];
        assert_eq!(percentile(&s, 50, 100), 30);
        assert_eq!(percentile(&s, 95, 100), 100);
        assert_eq!(percentile(&s, 99, 100), 100);
    }
}
