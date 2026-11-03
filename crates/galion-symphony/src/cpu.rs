//! Process CPU time for the P1 share.
//!
//! Prefer `/proc/self/schedstat` (nanoseconds on the CPU). Fall back to
//! `/proc/self/stat` utime+stime, which moves in `CLK_TCK` quanta.
//! Neither path is a hosted-SKU measurement. A missing procfs yields
//! [`None`] and the account leaves P1 unresolved rather than inventing a share.

use std::io::Read;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuSample {
    pub ns: u64,
    pub resolution_ns: u64,
    pub source: &'static str,
}

static CLK_TCK: OnceLock<u64> = OnceLock::new();

pub fn read() -> Option<CpuSample> {
    if let Some(ns) = read_schedstat() {
        return Some(CpuSample {
            ns,
            resolution_ns: 1,
            source: "schedstat",
        });
    }
    read_stat()
}

pub fn delta(start: CpuSample, end: CpuSample) -> Option<CpuSample> {
    if start.source != end.source || end.ns < start.ns {
        return None;
    }
    Some(CpuSample {
        ns: end.ns - start.ns,
        resolution_ns: end.resolution_ns.max(start.resolution_ns),
        source: end.source,
    })
}

fn read_schedstat() -> Option<u64> {
    // Thread, not the process: other test threads must not move this counter.
    // The kernel publishes the number on a schedule point, so the caller yields
    // before the closing read.
    for path in ["/proc/thread-self/schedstat", "/proc/self/schedstat"] {
        if let Some(ns) = read_schedstat_file(path) {
            return Some(ns);
        }
    }
    None
}

fn read_schedstat_file(path: &str) -> Option<u64> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 96];
    let n = f.read(&mut buf).ok()?;
    let text = std::str::from_utf8(&buf[..n]).ok()?;
    text.split_whitespace().next()?.parse().ok()
}

fn read_stat() -> Option<CpuSample> {
    let mut f = std::fs::File::open("/proc/self/stat").ok()?;
    let mut buf = [0u8; 1024];
    let n = f.read(&mut buf).ok()?;
    let text = std::str::from_utf8(&buf[..n]).ok()?;
    let after = text.rsplit_once(')')?.1;
    let mut fields = after.split_whitespace();
    // utime is field 14, stime field 15. `after` starts at field 3 (state).
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let hz = *CLK_TCK.get_or_init(probe_clk_tck);
    if hz == 0 {
        return None;
    }
    let ticks = utime.saturating_add(stime);
    let resolution_ns = 1_000_000_000u64 / hz;
    Some(CpuSample {
        ns: ticks.saturating_mul(resolution_ns),
        resolution_ns,
        source: "stat",
    })
}

fn probe_clk_tck() -> u64 {
    let Ok(out) = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
    else {
        return 100;
    };
    if !out.status.success() {
        return 100;
    }
    std::str::from_utf8(&out.stdout)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|hz: &u64| *hz > 0)
        .unwrap_or(100)
}
