// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Percentiles and the Linux resident set.
//!
//! `VmRSS` and smaps `Rss` are printed by the soak. The growth gate is live
//! heap bytes, measured by the test binary's allocator, not by these reads.

/// Nearest rank. `pct` is 50 or 99. An empty slice is refused by the caller.
#[must_use]
pub fn percentile(sorted: &[u64], pct: f64) -> u64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    let rank = ((pct / 100.0) * n as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(n - 1);
    sorted[index]
}

/// `VmRSS` in KiB from `/proc/self/status`.
///
/// Off Linux this is an error. The soak prints it and does not gate on it.
pub fn vm_rss_kib() -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/status")
            .map_err(|err| format!("reading /proc/self/status: {err}"))?;
        parse_vm_rss(&text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("VmRSS is read from /proc/self/status on Linux".to_string())
    }
}

pub fn parse_vm_rss(text: &str) -> Result<u64, String> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let Some(kb) = rest.split_whitespace().next() else {
            return Err("VmRSS has no number".to_string());
        };
        return kb
            .parse::<u64>()
            .map_err(|err| format!("VmRSS {kb}: {err}"));
    }
    Err("VmRSS is not in /proc/self/status".to_string())
}

/// Exact `Rss` in KiB from `/proc/self/smaps_rollup`.
///
/// Off Linux, or when the file cannot be read, this is an error. The soak
/// prints it and does not gate on it. `VmRSS` moves in page batches; this
/// sum is the resident pages themselves.
pub fn smaps_rss_kib() -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/smaps_rollup")
            .map_err(|err| format!("reading /proc/self/smaps_rollup: {err}"))?;
        parse_smaps_rss(&text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Rss is read from /proc/self/smaps_rollup on Linux".to_string())
    }
}

pub fn parse_smaps_rss(text: &str) -> Result<u64, String> {
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("Rss:") else {
            continue;
        };
        let Some(kb) = rest.split_whitespace().next() else {
            return Err("Rss has no number".to_string());
        };
        return kb.parse::<u64>().map_err(|err| format!("Rss {kb}: {err}"));
    }
    Err("Rss is not in /proc/self/smaps_rollup".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_picks_the_ceil_slot() {
        let samples = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(percentile(&samples, 50.0), 5);
        assert_eq!(percentile(&samples, 99.0), 10);
        assert_eq!(percentile(&samples, 100.0), 10);
    }

    #[test]
    fn vmrss_line_is_kib() {
        let text = "Name:\tsoak\nVmRSS:\t  20480 kB\nVmSize:\t  40000 kB\n";
        assert_eq!(parse_vm_rss(text).expect("rss"), 20_480);
    }

    #[test]
    fn a_missing_vmrss_line_is_an_error() {
        assert!(parse_vm_rss("Name:\tsoak\n").is_err());
    }

    #[test]
    fn smaps_rollup_rss_line_is_kib() {
        let text = "560000000000-7fffffffffff ---p 00000000 00:00 0 [rollup]\nRss:               41580 kB\nPss:               20000 kB\n";
        assert_eq!(parse_smaps_rss(text).expect("rss"), 41_580);
    }

    #[test]
    fn a_missing_smaps_rss_line_is_an_error() {
        assert!(parse_smaps_rss("Pss: 1 kB\n").is_err());
    }
}
