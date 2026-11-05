// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2 live-call core map, shared by every process launcher on the voice host.
//!
//! **v3** (owner ruling 2026-11-04 20:45 CET): the reasoning sector is fully
//! local and in-house. No Qwen, no llama-server, no Groq or other cloud LLM, and
//! no fallback to any of them. Reasoning is the S0 student (~0.5B, `.gq`, W8A8)
//! on `galion-tensor`, with `CannedLlm` until trained weights exist.
//!
//! | core | role | threads |
//! |---|---|---|
//! | 0 | bus / heartbeat / audio I/O / VAD-endpoint / orchestrator | 1 |
//! | 0 | retrieval / context worker (low priority, shares core 0) | 1 |
//! | 1 | Vosk ASR | 1 |
//! | 2-5 | in-house reasoning (S0 on galion-tensor) | 4 |
//! | 6 | Piper, Samantha voice | 1 |
//! | 7 | Piper, Jarvis voice | 1 |
//!
//! Clone workers get no cores during a live call (`ClonePlan::Off` in
//! galion-ml-voice).
//!
//! History (superseded, not in force): the earlier v3 A/B put a third-party
//! llama-server LLM on 2-5 and one Piper on 6-7 (2 threads). Those numbers are
//! kept in `docs/galion-voice-v2/latency-240/06-core-map-v3.md`, labelled
//! historical. **v2** (bus 0, Vosk 1, reasoning 2-5, Piper 6, retrieval 7)
//! stays selectable for A/B with `GALION_CORE_MAP=v2`.
//!
//! `GALION_CORE_MAP`: `v3` / `v2` / `off` / `auto` (default). `auto` turns on
//! v3 when `GALION_REASONING_LOCAL` (or the older `GALION_LLM_LOCAL`) is set
//! and the host has 8+ usable cores. `off` keeps every child on the inherited
//! mask. Per-role overrides: `GALION_CORES_<ROLE>` (`BUS`, `ASR`, `REASONING`,
//! `PIPER_SAMANTHA`, `PIPER_JARVIS`, `RETRIEVAL`), e.g. `4-7`, and
//! `GALION_THREADS_<ROLE>` (clamped to the role's cores).
#![allow(unsafe_code)]

use std::process::Command;

/// Cores the maps index (0-7).
pub const CORE_MAP_MIN_CPUS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreRole {
    Bus,
    Asr,
    /// In-house reasoning (S0 student on galion-tensor). Parses from `llm` too.
    Reasoning,
    /// Piper for the Samantha voice.
    PiperSamantha,
    /// Piper for the Jarvis voice.
    PiperJarvis,
    Retrieval,
}

/// Which table is in force.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreMap {
    Off,
    V2,
    V3,
}

impl CoreMap {
    pub fn as_str(self) -> &'static str {
        match self {
            CoreMap::Off => "off",
            CoreMap::V2 => "v2",
            CoreMap::V3 => "v3",
        }
    }
}

impl CoreRole {
    pub const ALL: [CoreRole; 6] = [
        CoreRole::Bus,
        CoreRole::Asr,
        CoreRole::Reasoning,
        CoreRole::PiperSamantha,
        CoreRole::PiperJarvis,
        CoreRole::Retrieval,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CoreRole::Bus => "bus",
            CoreRole::Asr => "asr",
            CoreRole::Reasoning => "reasoning",
            CoreRole::PiperSamantha => "piper_samantha",
            CoreRole::PiperJarvis => "piper_jarvis",
            CoreRole::Retrieval => "retrieval",
        }
    }

    /// Piper role for a voice: `jarvis` gets core 7, anything else core 6.
    pub fn piper_for_voice(voice: &str) -> Self {
        if voice.to_ascii_lowercase().contains("jarvis") {
            CoreRole::PiperJarvis
        } else {
            CoreRole::PiperSamantha
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        let s = match s.as_str() {
            "vosk" => "asr",
            "llm" => "reasoning",
            "piper" | "samantha" => "piper_samantha",
            "jarvis" => "piper_jarvis",
            other => other,
        };
        Self::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// The v2 map (one role per core). Kept for A/B.
    pub fn v2_cpus(self) -> Vec<usize> {
        match self {
            CoreRole::Bus => vec![0],
            CoreRole::Asr => vec![1],
            CoreRole::Reasoning => (2..6).collect(),
            CoreRole::PiperSamantha | CoreRole::PiperJarvis => vec![6],
            CoreRole::Retrieval => vec![7],
        }
    }

    /// The v3 map: bus + retrieval 0, Vosk 1, in-house reasoning 2-5,
    /// Piper Samantha 6, Piper Jarvis 7.
    pub fn v3_cpus(self) -> Vec<usize> {
        match self {
            CoreRole::Bus | CoreRole::Retrieval => vec![0],
            CoreRole::Asr => vec![1],
            CoreRole::Reasoning => (2..6).collect(),
            CoreRole::PiperSamantha => vec![6],
            CoreRole::PiperJarvis => vec![7],
        }
    }

    pub fn cpus_in(self, map: CoreMap) -> Vec<usize> {
        match map {
            CoreMap::Off => Vec::new(),
            CoreMap::V2 => self.v2_cpus(),
            CoreMap::V3 => self.v3_cpus(),
        }
    }
}

/// Resolved placement for one role: cores to pin to (empty = not pinned) and
/// the thread count its pools should use (never more than its cores; 0 = the
/// child's own default).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolePlacement {
    pub role: CoreRole,
    pub map: CoreMap,
    pub cpus: Vec<usize>,
    pub threads: u32,
}

impl RolePlacement {
    pub fn pinned(&self) -> bool {
        !self.cpus.is_empty()
    }
    pub fn cpu_list(&self) -> String {
        format_cpu_list(&self.cpus)
    }
}

/// The map in force on this host.
pub fn active_map() -> CoreMap {
    let get = |k: &str| std::env::var(k).ok();
    active_map_with(get("GALION_CORE_MAP").as_deref(), local_llm_env(), usable_cpus())
}

pub fn active_map_with(map: Option<&str>, local_llm: bool, usable_cpus: usize) -> CoreMap {
    if usable_cpus < CORE_MAP_MIN_CPUS {
        return CoreMap::Off;
    }
    match map.map(|m| m.trim().to_ascii_lowercase()).as_deref() {
        Some("v3") => CoreMap::V3,
        Some("v2") => CoreMap::V2,
        Some("off" | "none") => CoreMap::Off,
        _ if local_llm => CoreMap::V3,
        _ => CoreMap::Off,
    }
}

/// True when any map applies on this host.
pub fn core_map_active() -> bool {
    active_map() != CoreMap::Off
}

/// Back-compat for the v2 signature.
pub fn core_map_active_with(map: Option<&str>, local_llm: bool, usable_cpus: usize) -> bool {
    active_map_with(map, local_llm, usable_cpus) != CoreMap::Off
}

/// Placement for `role`, or an unpinned placement when the map is off.
pub fn placement(role: CoreRole) -> RolePlacement {
    let get = |k: &str| std::env::var(k).ok();
    let up = role.as_str().to_ascii_uppercase();
    placement_with(
        role,
        active_map(),
        get(&format!("GALION_CORES_{up}")).as_deref(),
        get(&format!("GALION_THREADS_{up}")).as_deref(),
        usable_cpus(),
    )
}

pub fn placement_with(
    role: CoreRole,
    map: CoreMap,
    cores_env: Option<&str>,
    threads_env: Option<&str>,
    usable_cpus: usize,
) -> RolePlacement {
    if map == CoreMap::Off {
        return RolePlacement { role, map, cpus: Vec::new(), threads: 0 };
    }
    let cpus = cores_env
        .and_then(parse_cpu_list)
        .filter(|c| c.iter().all(|&i| i < usable_cpus))
        .unwrap_or_else(|| role.cpus_in(map));
    let max = cpus.len() as u32;
    let threads = threads_env
        .and_then(|t| t.trim().parse::<u32>().ok())
        .map(|t| t.clamp(1, max.max(1)))
        .unwrap_or(max);
    RolePlacement { role, map, cpus, threads }
}

fn truthy(v: &str) -> bool {
    !matches!(v.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "no")
}

/// The in-house reasoning sector runs on this host: `GALION_REASONING_LOCAL`
/// (or the older `GALION_LLM_LOCAL`) is set to something truthy.
pub fn local_llm_env() -> bool {
    ["GALION_REASONING_LOCAL", "GALION_LLM_LOCAL"]
        .iter()
        .any(|k| std::env::var(k).is_ok_and(|v| truthy(&v)))
}

/// Usable cores for this process (respects an inherited affinity mask).
pub fn usable_cpus() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

pub const CPU_SET_BITS: usize = 1024;

/// Parse `0-3`, `0,2,4`, `0-2,6`. None on anything else.
pub fn parse_cpu_list(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?);
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
        out.push(if i == j { cpus[i].to_string() } else { format!("{}-{}", cpus[i], cpus[j]) });
        i = j + 1;
    }
    out.join(",")
}

extern "C" {
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
}

fn mask_of(cpus: &[usize]) -> [u64; CPU_SET_BITS / 64] {
    let mut mask = [0u64; CPU_SET_BITS / 64];
    for &c in cpus {
        if c < CPU_SET_BITS {
            mask[c / 64] |= 1u64 << (c % 64);
        }
    }
    mask
}

/// Pin a child to `cpus` with `sched_setaffinity` after fork, before exec.
/// Empty `cpus` leaves the inherited mask.
pub fn apply_affinity(cmd: &mut Command, cpus: &[usize]) {
    if cpus.is_empty() {
        return;
    }
    let mask = mask_of(cpus);
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

/// Pin the **calling thread** (pid 0 = this thread on Linux) to `cpus`.
/// Threads it spawns afterwards inherit the mask. Empty `cpus` is a no-op.
pub fn pin_current_thread(cpus: &[usize]) -> std::io::Result<()> {
    if cpus.is_empty() {
        return Ok(());
    }
    let mask = mask_of(cpus);
    // SAFETY: plain syscall on a stack mask of the declared size.
    if unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Pin the calling thread to `role`'s placement. Returns the placement used.
pub fn pin_current_thread_to(role: CoreRole) -> std::io::Result<RolePlacement> {
    let p = placement(role);
    pin_current_thread(&p.cpus)?;
    Ok(p)
}

/// This thread's allowed cores, from `/proc/thread-self/status`.
pub fn current_cpus() -> Option<Vec<usize>> {
    let s = std::fs::read_to_string("/proc/thread-self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("Cpus_allowed_list:"))?;
    parse_cpu_list(line.split_once(':')?.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_map_is_kept_for_ab() {
        assert_eq!(CoreRole::PiperSamantha.v2_cpus(), vec![6]);
        assert_eq!(CoreRole::Retrieval.v2_cpus(), vec![7]);
        assert_eq!(format_cpu_list(&CoreRole::Reasoning.v2_cpus()), "2-5");
    }

    #[test]
    fn v3_map_is_the_owner_map() {
        assert_eq!(CoreRole::Bus.v3_cpus(), vec![0]);
        assert_eq!(CoreRole::Retrieval.v3_cpus(), vec![0], "retrieval shares core 0, low priority");
        assert_eq!(CoreRole::Asr.v3_cpus(), vec![1]);
        assert_eq!(format_cpu_list(&CoreRole::Reasoning.v3_cpus()), "2-5");
        assert_eq!(CoreRole::Reasoning.v3_cpus().len(), 4, "S0 on galion-tensor, 4 threads");
        assert_eq!(CoreRole::PiperSamantha.v3_cpus(), vec![6]);
        assert_eq!(CoreRole::PiperJarvis.v3_cpus(), vec![7]);
        // Vosk, reasoning and each Piper never share a core with anything.
        for solo in [CoreRole::Asr, CoreRole::Reasoning, CoreRole::PiperSamantha, CoreRole::PiperJarvis] {
            for other in CoreRole::ALL.into_iter().filter(|r| *r != solo) {
                assert!(solo.v3_cpus().iter().all(|c| !other.v3_cpus().contains(c)), "{solo:?} vs {other:?}");
            }
        }
        let mut all: Vec<usize> = CoreRole::ALL.iter().flat_map(|r| r.v3_cpus()).collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all, (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn role_names() {
        assert_eq!(CoreRole::parse("llm"), Some(CoreRole::Reasoning));
        assert_eq!(CoreRole::parse("piper"), Some(CoreRole::PiperSamantha));
        assert_eq!(CoreRole::parse("jarvis"), Some(CoreRole::PiperJarvis));
        assert_eq!(CoreRole::piper_for_voice("voices/jarvis/voice.onnx"), CoreRole::PiperJarvis);
        assert_eq!(CoreRole::piper_for_voice("voices/samantha/voice.onnx"), CoreRole::PiperSamantha);
    }

    #[test]
    fn map_switch() {
        assert_eq!(active_map_with(Some("v2"), false, 8), CoreMap::V2);
        assert_eq!(active_map_with(Some("v3"), false, 8), CoreMap::V3);
        assert_eq!(active_map_with(Some("v3"), true, 4), CoreMap::Off, "small host never pins");
        assert_eq!(active_map_with(Some("off"), true, 8), CoreMap::Off);
        assert_eq!(active_map_with(None, true, 8), CoreMap::V3, "auto + local reasoning = v3");
        assert_eq!(active_map_with(Some("auto"), true, 8), CoreMap::V3);
        assert_eq!(active_map_with(None, false, 8), CoreMap::Off);
        assert!(core_map_active_with(Some("v2"), false, 8));
    }

    #[test]
    fn overrides_and_thread_clamp() {
        let p = placement_with(CoreRole::PiperJarvis, CoreMap::V3, None, None, 8);
        assert_eq!((p.cpu_list(), p.threads), ("7".into(), 1));
        let p = placement_with(CoreRole::PiperSamantha, CoreMap::V3, Some("5-7"), Some("9"), 8);
        assert_eq!((p.cpu_list(), p.threads), ("5-7".into(), 3), "threads clamp to cores");
        let p = placement_with(CoreRole::Reasoning, CoreMap::V3, Some("4-11"), None, 8);
        assert_eq!(p.cpu_list(), "2-5", "out-of-range override ignored");
        let p = placement_with(CoreRole::Reasoning, CoreMap::Off, Some("0"), Some("2"), 8);
        assert!(!p.pinned() && p.threads == 0);
        assert_eq!(CoreRole::parse("vosk"), Some(CoreRole::Asr));
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
        cmd.arg("-c").arg("grep Cpus_allowed_list /proc/self/status");
        apply_affinity(&mut cmd, &[1]);
        let out = cmd.output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.trim_end().ends_with("\t1"), "{text}");
    }

    #[test]
    fn current_thread_pin_is_per_thread() {
        if usable_cpus() < 2 {
            return;
        }
        let before = current_cpus().unwrap();
        let inside = std::thread::spawn(|| {
            pin_current_thread(&[0]).unwrap();
            current_cpus().unwrap()
        })
        .join()
        .unwrap();
        assert_eq!(inside, vec![0]);
        assert_eq!(current_cpus().unwrap(), before, "other threads keep their mask");
    }
}
