// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2 live-call core map (latency-240, agent 02; agreed with the Symphony
//! plan and agent 05). One role per core on the 8-core voice host, fully
//! local, no clone workers during a live call, Piper is the TTS engine:
//!
//! | core | role |
//! |---|---|
//! | 0 | heartbeat / bus |
//! | 1 | Vosk ASR |
//! | 2-5 | local LLM (Qwen2.5-1.5B, 4 threads) |
//! | 6 | Piper |
//! | 7 | retrieval |
//!
//! Active with `GALION_CORE_MAP=v2`, or `auto` (the default) when
//! `GALION_LLM_LOCAL` is set and the host has 8+ usable cores. `off` keeps
//! every child on the inherited mask. Per-role overrides:
//! `GALION_CORES_<ROLE>` (`BUS`, `ASR`, `LLM`, `PIPER`, `RETRIEVAL`), e.g. `2-5`.
#![allow(unsafe_code)]

use std::process::Command;

/// Cores the v2 map indexes (0-7).
pub const CORE_MAP_MIN_CPUS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreRole {
    Bus,
    Asr,
    Llm,
    Piper,
    Retrieval,
}

impl CoreRole {
    pub const ALL: [CoreRole; 5] = [
        CoreRole::Bus,
        CoreRole::Asr,
        CoreRole::Llm,
        CoreRole::Piper,
        CoreRole::Retrieval,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CoreRole::Bus => "bus",
            CoreRole::Asr => "asr",
            CoreRole::Llm => "llm",
            CoreRole::Piper => "piper",
            CoreRole::Retrieval => "retrieval",
        }
    }

    /// The v2 map's cores for this role.
    pub fn v2_cpus(self) -> Vec<usize> {
        match self {
            CoreRole::Bus => vec![0],
            CoreRole::Asr => vec![1],
            CoreRole::Llm => (2..6).collect(),
            CoreRole::Piper => vec![6],
            CoreRole::Retrieval => vec![7],
        }
    }
}

/// Resolved placement for one role: cores to pin to (empty = not pinned) and
/// the thread count its pools should use (never more than its cores).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolePlacement {
    pub role: CoreRole,
    pub cpus: Vec<usize>,
    pub threads: u32,
}

/// True when the v2 core map applies on this host.
pub fn core_map_active() -> bool {
    let get = |k: &str| std::env::var(k).ok();
    let local = get("GALION_LLM_LOCAL").is_some_and(|v| truthy(&v));
    core_map_active_with(get("GALION_CORE_MAP").as_deref(), local, usable_cpus())
}

pub fn core_map_active_with(map: Option<&str>, local_llm: bool, usable_cpus: usize) -> bool {
    if usable_cpus < CORE_MAP_MIN_CPUS {
        return false;
    }
    match map.map(|m| m.trim().to_ascii_lowercase()) {
        Some(m) if m == "v2" => true,
        Some(m) if m == "off" || m == "none" => false,
        _ => local_llm,
    }
}

/// Placement for `role`, or an unpinned placement when the map is off.
pub fn placement(role: CoreRole) -> RolePlacement {
    if !core_map_active() {
        return RolePlacement {
            role,
            cpus: Vec::new(),
            threads: 0,
        };
    }
    let n = usable_cpus();
    let key = format!("GALION_CORES_{}", role.as_str().to_ascii_uppercase());
    let cpus = std::env::var(key)
        .ok()
        .and_then(|v| parse_cpu_list(&v))
        .filter(|c| c.iter().all(|&i| i < n))
        .unwrap_or_else(|| role.v2_cpus());
    let threads = cpus.len() as u32;
    RolePlacement {
        role,
        cpus,
        threads,
    }
}

fn truthy(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no"
    )
}

pub(crate) fn local_llm_env() -> bool {
    std::env::var("GALION_LLM_LOCAL").is_ok_and(|v| truthy(&v))
}

/// Usable cores for this process (respects an inherited affinity mask).
pub fn usable_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub(crate) const CPU_SET_BITS: usize = 1024;

/// Parse `0-3`, `0,2,4`, `0-2,6`. None on anything else.
pub fn parse_cpu_list(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (
                    a.trim().parse::<usize>().ok()?,
                    b.trim().parse::<usize>().ok()?,
                );
                if a > b || b >= CPU_SET_BITS {
                    return None;
                }
                v.extend(a..=b);
            }
            None => {
                let i = part.parse::<usize>().ok()?;
                if i >= CPU_SET_BITS {
                    return None;
                }
                v.push(i);
            }
        }
    }
    v.sort_unstable();
    v.dedup();
    (!v.is_empty()).then_some(v)
}

/// taskset-style list, e.g. `0-2,6`.
pub fn format_cpu_list(cpus: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cpus.len() {
        let mut j = i;
        while j + 1 < cpus.len() && cpus[j + 1] == cpus[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            cpus[i].to_string()
        } else {
            format!("{}-{}", cpus[i], cpus[j])
        });
        i = j + 1;
    }
    out.join(",")
}

extern "C" {
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
}

/// Pin a child to `cpus` with `sched_setaffinity` after fork, before exec.
/// Empty `cpus` leaves the inherited mask.
pub fn apply_affinity(cmd: &mut Command, cpus: &[usize]) {
    if cpus.is_empty() {
        return;
    }
    let mut mask = [0u64; CPU_SET_BITS / 64];
    for &c in cpus {
        mask[c / 64] |= 1u64 << (c % 64);
    }
    use std::os::unix::process::CommandExt;
    // SAFETY: the closure runs in the forked child before exec and only makes
    // one async-signal-safe syscall on a mask it owns by value.
    unsafe {
        cmd.pre_exec(move || {
            if sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_map_gives_every_role_its_own_cores() {
        let mut seen = Vec::new();
        for r in CoreRole::ALL {
            for c in r.v2_cpus() {
                assert!(c < CORE_MAP_MIN_CPUS);
                assert!(!seen.contains(&c), "core {c} has two roles");
                seen.push(c);
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, (0..8).collect::<Vec<_>>(), "all 8 cores assigned");
        assert_eq!(format_cpu_list(&CoreRole::Llm.v2_cpus()), "2-5");
        assert_eq!(CoreRole::Llm.v2_cpus().len(), 4, "agent 05: 4 LLM threads");
        assert_eq!(CoreRole::Piper.v2_cpus(), vec![6]);
        assert_eq!(CoreRole::Asr.v2_cpus(), vec![1]);
    }

    #[test]
    fn map_switch() {
        assert!(core_map_active_with(Some("v2"), false, 8));
        assert!(
            !core_map_active_with(Some("v2"), true, 4),
            "small host never pins"
        );
        assert!(!core_map_active_with(Some("off"), true, 8));
        assert!(core_map_active_with(None, true, 8));
        assert!(core_map_active_with(Some("auto"), true, 8));
        assert!(!core_map_active_with(None, false, 8));
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(parse_cpu_list("0-2,6"), Some(vec![0, 1, 2, 6]));
        assert_eq!(parse_cpu_list("3-1"), None);
        assert_eq!(parse_cpu_list("x"), None);
        assert_eq!(format_cpu_list(&[0, 1, 2, 6]), "0-2,6");
    }

    #[test]
    fn affinity_lands_in_the_child_before_exec() {
        if usable_cpus() < 2 {
            return;
        }
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("grep Cpus_allowed_list /proc/self/status");
        apply_affinity(&mut cmd, &[1]);
        let out = cmd.output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.trim_end().ends_with("\t1"), "{text}");
    }
}
