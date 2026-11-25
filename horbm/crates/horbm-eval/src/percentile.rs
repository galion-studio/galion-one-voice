// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Linear percentile. Rank is `p/100 * (n - 1)`, the same interpolation the
//! end-of-turn notes use. One sample is that sample. Empty is absent.

/// `p` is 0 to 100. Returns `None` when `samples` is empty.
#[must_use]
pub fn percentile(samples: &[f64], p: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let rank = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] * (1.0 - frac) + sorted[hi] * frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_linear_rank() {
        assert!(percentile(&[], 50.0).is_none());
        assert_eq!(percentile(&[4.0], 95.0), Some(4.0));
        let samples = [0.0, 10.0, 20.0, 30.0];
        let p50 = percentile(&samples, 50.0).expect("p50");
        let p95 = percentile(&samples, 95.0).expect("p95");
        assert!((p50 - 15.0).abs() < 1.0e-9);
        assert!((p95 - 28.5).abs() < 1.0e-9);
    }
}
