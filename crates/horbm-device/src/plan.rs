// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pure plan from one hardware sample.
//!
//! Class, thread counts, and the compute budget are a function of the sample.
//! They do not read the clock, the filesystem, or an environment variable.

use std::fmt;

/// Profile schema. A file with another version is rejected.
pub const PROFILE_VERSION: u16 = 1;

/// Locked frame. `LOCKS.toml` `clock.frame_ms`. Not configurable.
pub const FRAME_MS: u32 = 80;

/// Locked compute cap inside the frame. `LOCKS.toml` `clock.frame_budget_p95_ms`.
pub const FRAME_BUDGET_P95_MS: u32 = 60;

/// Logical processors at or below this are a phone, before RAM is considered.
pub const PHONE_MAX_CORES: u32 = 2;

/// Logical processors at or below this are a small board, before RAM is considered.
pub const BOARD_MAX_CORES: u32 = 4;

/// When RAM was readable, a total under this is a phone. Policy, not a measurement.
pub const PHONE_RAM_MIB: u64 = 2048;

/// When RAM was readable, a total under this is a small board.
///
/// The same 8192 MiB figure is the Open Mind mouth flag. Here it only picks a
/// class. A board still runs the CPU brain.
pub const BOARD_RAM_MIB: u64 = 8192;

/// Worker pool cap. The v3 core map gives reasoning four threads (cores 2–5).
/// This crate does not pin those cores.
pub const WORKER_CAP: u32 = 4;

/// Cores that share [`FRAME_BUDGET_P95_MS`]. Past this the budget stays at the cap.
pub const BUDGET_CORE_SPAN: u32 = 4;

/// One retrieval thread. The v3 map shares it with the bus.
pub const RETRIEVAL_THREADS: u32 = 1;

/// One mouth thread for the active voice.
pub const MOUTH_THREADS: u32 = 1;

/// Where the sample sits. A workstation uses [`DeviceClass::Laptop`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    /// At most two logical processors, or under [`PHONE_RAM_MIB`] when RAM was read.
    Phone,
    /// At most four logical processors, or under [`BOARD_RAM_MIB`] when RAM was read.
    Board,
    /// Anything larger that still has a processor.
    Laptop,
}

impl DeviceClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            DeviceClass::Phone => "phone",
            DeviceClass::Board => "board",
            DeviceClass::Laptop => "laptop",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "phone" => Some(DeviceClass::Phone),
            "board" => Some(DeviceClass::Board),
            "laptop" => Some(DeviceClass::Laptop),
            _ => None,
        }
    }
}

impl fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A backend named in the profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Gpu,
}

impl Backend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Gpu => "gpu",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "cpu" => Some(Backend::Cpu),
            "gpu" => Some(Backend::Gpu),
            _ => None,
        }
    }
}

/// Whether compute may leave the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuChoice {
    /// CPU compute. A detected GPU is written into the profile and not selected.
    Cpu,
    /// GPU compute when the sample has a GPU. CPU when it does not.
    OptIn,
}

/// Who built the optional GPU row. Version 1 records NVIDIA only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
}

impl GpuVendor {
    pub const fn as_str(self) -> &'static str {
        match self {
            GpuVendor::Nvidia => "nvidia",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "nvidia" => Some(GpuVendor::Nvidia),
            _ => None,
        }
    }
}

/// One GPU, as the probe reported it. MiB are integer bytes / 1024 / 1024.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub name: String,
    pub total_mib: u64,
    pub free_mib: u64,
}

/// What a probe returns. [`crate::plan`] does not look at anything else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HardwareSample {
    /// Logical processors. Zero means the probe could not count them.
    pub cores: u32,
    /// Total RAM in MiB. Zero means the probe could not read it.
    pub ram_mib: u64,
    pub arch: String,
    pub avx2: bool,
    pub neon: bool,
    pub gpu: Option<GpuInfo>,
}

/// Pool sizes a layer should use. Not a core pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadPlan {
    pub retrieval: u32,
    pub mouth: u32,
    pub workers: u32,
}

/// Versioned plan for one boot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    pub version: u16,
    pub class: DeviceClass,
    /// Probe count, including zero when the count was unknown.
    pub cores: u32,
    pub ram_mib: u64,
    pub arch: String,
    pub avx2: bool,
    pub neon: bool,
    /// Reported even when [`Self::compute`] stays on the CPU.
    pub gpu: Option<GpuInfo>,
    /// Always [`Backend::Cpu`].
    pub brain: Backend,
    /// Always [`Backend::Cpu`]. The mouth does not follow the GPU flag.
    pub mouth: Backend,
    /// [`Backend::Gpu`] only after [`GpuChoice::OptIn`] and a detected GPU.
    pub compute: Backend,
    pub threads: ThreadPlan,
    /// Always [`FRAME_MS`].
    pub frame_ms: u32,
    /// Share of [`FRAME_BUDGET_P95_MS`]. Never above that cap.
    pub compute_budget_ms: u32,
}

/// Build the profile. Deterministic for a given sample and choice.
pub fn plan(sample: &HardwareSample, choice: GpuChoice) -> DeviceProfile {
    let class = classify(sample.cores, sample.ram_mib);
    let gpu = sample.gpu.as_ref().map(sanitize_gpu);
    let compute = match choice {
        GpuChoice::Cpu => Backend::Cpu,
        GpuChoice::OptIn if gpu.is_some() => Backend::Gpu,
        GpuChoice::OptIn => Backend::Cpu,
    };
    DeviceProfile {
        version: PROFILE_VERSION,
        class,
        cores: sample.cores,
        ram_mib: sample.ram_mib,
        arch: sanitize_arch(&sample.arch),
        avx2: sample.avx2,
        neon: sample.neon,
        gpu,
        brain: Backend::Cpu,
        mouth: Backend::Cpu,
        compute,
        threads: ThreadPlan {
            retrieval: RETRIEVAL_THREADS,
            mouth: MOUTH_THREADS,
            workers: worker_threads(class, sample.cores),
        },
        frame_ms: FRAME_MS,
        compute_budget_ms: compute_budget_ms(sample.cores),
    }
}

fn usable_cores(cores: u32) -> u32 {
    cores.max(1)
}

fn classify(cores: u32, ram_mib: u64) -> DeviceClass {
    let cores = usable_cores(cores);
    let ram_known = ram_mib > 0;
    if cores <= PHONE_MAX_CORES || (ram_known && ram_mib < PHONE_RAM_MIB) {
        DeviceClass::Phone
    } else if cores <= BOARD_MAX_CORES || (ram_known && ram_mib < BOARD_RAM_MIB) {
        DeviceClass::Board
    } else {
        DeviceClass::Laptop
    }
}

fn worker_threads(class: DeviceClass, cores: u32) -> u32 {
    let cores = usable_cores(cores);
    match class {
        DeviceClass::Phone => 1,
        DeviceClass::Board => cores.min(2),
        DeviceClass::Laptop => cores.saturating_sub(1).clamp(1, WORKER_CAP),
    }
}

fn compute_budget_ms(cores: u32) -> u32 {
    let n = usable_cores(cores).min(BUDGET_CORE_SPAN);
    FRAME_BUDGET_P95_MS * n / BUDGET_CORE_SPAN
}

fn sanitize_arch(arch: &str) -> String {
    let ok = !arch.is_empty()
        && arch.len() <= 32
        && arch
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if ok {
        arch.to_string()
    } else {
        "unknown".to_string()
    }
}

fn sanitize_gpu(gpu: &GpuInfo) -> GpuInfo {
    GpuInfo {
        vendor: gpu.vendor,
        name: sanitize_gpu_name(&gpu.name),
        total_mib: gpu.total_mib,
        free_mib: gpu.free_mib,
    }
}

pub(crate) fn sanitize_gpu_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() >= 128 {
            break;
        }
        if (' '..='~').contains(&ch) {
            out.push(ch);
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "NVIDIA".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cores: u32, ram_mib: u64, gpu: Option<GpuInfo>) -> HardwareSample {
        HardwareSample {
            cores,
            ram_mib,
            arch: "x86_64".to_string(),
            avx2: true,
            neon: false,
            gpu,
        }
    }

    fn card(name: &str) -> GpuInfo {
        GpuInfo {
            vendor: GpuVendor::Nvidia,
            name: name.to_string(),
            total_mib: 8192,
            free_mib: 7000,
        }
    }

    #[test]
    fn two_cores_are_a_phone_and_eight_cores_are_a_laptop() {
        let phone = plan(&sample(2, 16384, None), GpuChoice::Cpu);
        assert_eq!(phone.class, DeviceClass::Phone);
        assert_eq!(phone.threads.retrieval, 1);
        assert_eq!(phone.threads.mouth, 1);
        assert_eq!(phone.threads.workers, 1);
        assert_eq!(phone.compute_budget_ms, 30);
        assert_eq!(phone.frame_ms, FRAME_MS);
        assert_eq!(phone.brain, Backend::Cpu);
        assert_eq!(phone.mouth, Backend::Cpu);
        assert_eq!(phone.compute, Backend::Cpu);

        let laptop = plan(&sample(8, 16384, None), GpuChoice::Cpu);
        assert_eq!(laptop.class, DeviceClass::Laptop);
        assert_eq!(laptop.threads.workers, 4);
        assert_eq!(laptop.compute_budget_ms, FRAME_BUDGET_P95_MS);
    }

    #[test]
    fn ram_cuts_class_only_when_the_probe_read_a_total() {
        assert_eq!(
            plan(&sample(8, 1024, None), GpuChoice::Cpu).class,
            DeviceClass::Phone
        );
        assert_eq!(
            plan(&sample(8, 2048, None), GpuChoice::Cpu).class,
            DeviceClass::Board
        );
        assert_eq!(
            plan(&sample(8, 4096, None), GpuChoice::Cpu).class,
            DeviceClass::Board
        );
        assert_eq!(
            plan(&sample(8, 8192, None), GpuChoice::Cpu).class,
            DeviceClass::Laptop
        );
        assert_eq!(
            plan(&sample(8, 0, None), GpuChoice::Cpu).class,
            DeviceClass::Laptop
        );
        assert_eq!(
            plan(&sample(4, 16384, None), GpuChoice::Cpu).class,
            DeviceClass::Board
        );
        assert_eq!(
            plan(&sample(3, 2047, None), GpuChoice::Cpu).class,
            DeviceClass::Phone
        );
    }

    #[test]
    fn an_unknown_count_still_plans_one_processor() {
        let profile = plan(&sample(0, 0, None), GpuChoice::OptIn);
        assert_eq!(profile.class, DeviceClass::Phone);
        assert_eq!(profile.cores, 0);
        assert_eq!(profile.threads.workers, 1);
        assert_eq!(profile.compute_budget_ms, 15);
        assert_eq!(profile.compute, Backend::Cpu);
        assert_eq!(profile.version, PROFILE_VERSION);
    }

    #[test]
    fn a_large_host_keeps_the_worker_cap_and_the_budget_cap() {
        let profile = plan(&sample(128, 262144, None), GpuChoice::Cpu);
        assert_eq!(profile.class, DeviceClass::Laptop);
        assert_eq!(profile.threads.workers, WORKER_CAP);
        assert_eq!(profile.compute_budget_ms, FRAME_BUDGET_P95_MS);
        assert!(profile.compute_budget_ms <= FRAME_BUDGET_P95_MS);
    }

    #[test]
    fn the_budget_follows_cores_inside_the_lock() {
        assert_eq!(
            plan(&sample(1, 16384, None), GpuChoice::Cpu).compute_budget_ms,
            15
        );
        assert_eq!(
            plan(&sample(2, 16384, None), GpuChoice::Cpu).compute_budget_ms,
            30
        );
        assert_eq!(
            plan(&sample(3, 8192, None), GpuChoice::Cpu).compute_budget_ms,
            45
        );
        assert_eq!(
            plan(&sample(4, 16384, None), GpuChoice::Cpu).compute_budget_ms,
            60
        );
        assert_eq!(
            plan(&sample(5, 16384, None), GpuChoice::Cpu).compute_budget_ms,
            60
        );
    }

    #[test]
    fn a_gpu_is_reported_and_the_brain_stays_on_cpu() {
        let with = plan(&sample(8, 16384, Some(card("Example"))), GpuChoice::Cpu);
        let without = plan(&sample(8, 16384, None), GpuChoice::Cpu);
        assert_eq!(with.brain, Backend::Cpu);
        assert_eq!(without.brain, Backend::Cpu);
        assert_eq!(with.mouth, Backend::Cpu);
        assert_eq!(without.mouth, Backend::Cpu);
        assert_eq!(with.compute, Backend::Cpu);
        assert_eq!(without.compute, Backend::Cpu);
        assert_eq!(
            with.gpu.as_ref().map(|gpu| gpu.name.as_str()),
            Some("Example")
        );
        assert!(without.gpu.is_none());
        assert_eq!(with.threads, without.threads);
    }

    #[test]
    fn gpu_opt_in_uses_the_card_and_falls_back_when_it_is_absent() {
        let opted = plan(&sample(8, 16384, Some(card("Example"))), GpuChoice::OptIn);
        assert_eq!(opted.compute, Backend::Gpu);
        assert_eq!(opted.brain, Backend::Cpu);
        assert_eq!(opted.mouth, Backend::Cpu);
        let missed = plan(&sample(8, 16384, None), GpuChoice::OptIn);
        assert_eq!(missed.compute, Backend::Cpu);
        assert_eq!(missed.brain, Backend::Cpu);
    }

    #[test]
    fn the_same_sample_plans_the_same_profile() {
        let sample = sample(6, 4096, Some(card("Example Card")));
        let first = plan(&sample, GpuChoice::OptIn);
        let second = plan(&sample, GpuChoice::OptIn);
        assert_eq!(first, second);
        assert_eq!(first.class, DeviceClass::Board);
        assert_eq!(first.threads.workers, 2);
    }

    #[test]
    fn a_dirty_gpu_name_and_arch_are_stored_clean() {
        let mut sample = sample(8, 16384, Some(card("  Bad\nName\u{0007}  ")));
        sample.arch = "x86 64".to_string();
        let profile = plan(&sample, GpuChoice::Cpu);
        assert_eq!(profile.gpu.unwrap().name, "BadName");
        assert_eq!(profile.arch, "unknown");
        let empty = plan(&sample_with_name(""), GpuChoice::Cpu);
        assert_eq!(empty.gpu.unwrap().name, "NVIDIA");
    }

    fn sample_with_name(name: &str) -> HardwareSample {
        sample(8, 16384, Some(card(name)))
    }
}
