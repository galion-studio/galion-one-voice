// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Host block stored on every results file: machine, CPU, date, commit.
//! Hostnames that look like IP addresses are stored as `redacted`.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub os: String,
    pub arch: String,
    pub kernel: String,
    pub cpu: String,
    pub cpu_mhz: String,
    pub nproc: u32,
    pub hostname: String,
    pub loadavg: String,
    pub date_utc: String,
    pub commit: String,
    pub commit_dirty: Option<bool>,
    pub rustc: String,
}

impl Machine {
    pub fn fixture() -> Self {
        Self {
            os: "test".into(),
            arch: "test".into(),
            kernel: "test".into(),
            cpu: "test-cpu".into(),
            cpu_mhz: "0".into(),
            nproc: 1,
            hostname: "test-host".into(),
            loadavg: "0.00 0.00 0.00".into(),
            date_utc: "2026-11-21T00:00:00Z".into(),
            commit: "abc".into(),
            commit_dirty: Some(false),
            rustc: "test".into(),
        }
    }
}

/// Repo root: the directory that contains `scripts/a21/fetch-a21.sh`.
/// Falls back to this crate's directory when the harness is copied alone.
pub fn repo_root() -> std::path::PathBuf {
    let mut cur = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if cur.join("scripts/a21/fetch-a21.sh").is_file() {
            return cur;
        }
        if !cur.pop() {
            break;
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn probe_machine(repo: &Path) -> Machine {
    let (kernel, uname_arch) = uname();
    let (cpu, mhz) = cpu_model();
    let nproc = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(0);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (commit, dirty) = git_head(repo);
    Machine {
        os: std::env::consts::OS.to_string(),
        arch: if uname_arch.is_empty() {
            std::env::consts::ARCH.to_string()
        } else {
            uname_arch
        },
        kernel,
        cpu,
        cpu_mhz: mhz,
        nproc,
        hostname: redact_host(&hostname()),
        loadavg: loadavg(),
        date_utc: format_unix_utc(secs),
        commit,
        commit_dirty: dirty,
        rustc: rustc_version(),
    }
}

pub fn redact_host(name: &str) -> String {
    let t = name.trim();
    if t.is_empty() {
        return "unknown".into();
    }
    if looks_like_ip(t) {
        return "redacted".into();
    }
    t.to_string()
}

fn looks_like_ip(s: &str) -> bool {
    if s.contains(':')
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == ':')
        && s.matches(':').count() >= 2
    {
        return true;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.len() <= 3
            && p.chars().all(|c| c.is_ascii_digit())
            && p.parse::<u8>().is_ok()
    })
}

pub fn format_unix_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let h = tod / 3_600;
    let m = (tod % 3_600) / 60;
    let s = tod % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Howard Hinnant `civil_from_days`. `z` is days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

fn cpu_model() -> (String, String) {
    if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
        let mut name = String::new();
        let mut mhz = String::new();
        for line in text.lines() {
            if name.is_empty() {
                if let Some(rest) = line.strip_prefix("model name") {
                    name = rest.split(':').nth(1).unwrap_or("").trim().to_string();
                }
            }
            if mhz.is_empty() {
                if let Some(rest) = line.strip_prefix("cpu MHz") {
                    mhz = rest.split(':').nth(1).unwrap_or("").trim().to_string();
                }
            }
            if !name.is_empty() && !mhz.is_empty() {
                break;
            }
        }
        if !name.is_empty() {
            return (
                name,
                if mhz.is_empty() {
                    "unknown".into()
                } else {
                    mhz
                },
            );
        }
    }
    if let Ok(id) = std::env::var("PROCESSOR_IDENTIFIER") {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return (id, "unknown".into());
        }
    }
    ("unknown".into(), "unknown".into())
}

fn loadavg() -> String {
    if let Ok(text) = std::fs::read_to_string("/proc/loadavg") {
        let mut parts = text.split_whitespace();
        let a = parts.next().unwrap_or("?");
        let b = parts.next().unwrap_or("?");
        let c = parts.next().unwrap_or("?");
        return format!("{a} {b} {c}");
    }
    "unknown".into()
}

fn hostname() -> String {
    if let Ok(text) = std::fs::read_to_string("/etc/hostname") {
        let t = text.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    command_stdout("uname", &["-n"]).unwrap_or_default()
}

fn uname() -> (String, String) {
    // "Linux 6.12.113+ x86_64" from `uname -srm` is kernel then arch, not a stable split
    // of three fields when the kernel has no spaces. `uname -r` and `uname -m` are one field each.
    let kernel = command_stdout("uname", &["-r"]).unwrap_or_else(|| "unknown".into());
    let arch = command_stdout("uname", &["-m"]).unwrap_or_default();
    (kernel, arch)
}

fn rustc_version() -> String {
    command_stdout("rustc", &["--version"]).unwrap_or_else(|| "unknown".into())
}

fn git_head(repo: &Path) -> (String, Option<bool>) {
    let commit = command_in(repo, "git", &["rev-parse", "HEAD"]);
    let dirty = command_in(repo, "git", &["status", "--porcelain"]).map(|s| !s.trim().is_empty());
    (commit.unwrap_or_else(|| "unknown".into()), dirty)
}

fn command_stdout(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn command_in(dir: &Path, bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_and_a_known_instant() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn ip_hostnames_are_redacted() {
        assert_eq!(redact_host("10.1.2.3"), "redacted");
        assert_eq!(redact_host("2001:db8::1"), "redacted");
        assert_eq!(redact_host("cursor"), "cursor");
        assert_eq!(redact_host("  "), "unknown");
        assert_eq!(redact_host("not.an.ip.address"), "not.an.ip.address");
    }
}
