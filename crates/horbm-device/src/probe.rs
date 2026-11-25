// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Probes. [`FixedProbe`] is the deterministic stand-in. [`HostProbe`] reads
//! the machine once per call and does not cache the sample.

use crate::plan::{GpuInfo, HardwareSample};

#[cfg(all(feature = "nvml", target_os = "linux"))]
use crate::plan::GpuVendor;

/// Something that can describe a machine. Tests pass [`FixedProbe`].
pub trait Probe {
    fn sample(&self) -> HardwareSample;
}

/// A sample the caller already has. `boot` of the same value does not read the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedProbe {
    pub sample: HardwareSample,
}

impl Probe for FixedProbe {
    fn sample(&self) -> HardwareSample {
        self.sample.clone()
    }
}

/// Live machine. Logical processors and total RAM come from `sysinfo`.
/// A cgroup quota is not applied. AVX2 and NEON come from `std::arch`.
/// The GPU row is the Linux NVML probe when that feature is compiled, and
/// absent otherwise.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostProbe;

impl HostProbe {
    pub fn sample(&self) -> HardwareSample {
        let (listed, bytes) = listed_cores_and_ram();
        let parallelism = std::thread::available_parallelism().ok().map(|n| n.get());
        HardwareSample {
            cores: cores_from_counts(listed, parallelism),
            ram_mib: ram_mib_from_bytes(bytes),
            arch: std::env::consts::ARCH.to_string(),
            avx2: host_avx2(),
            neon: host_neon(),
            gpu: detect_gpu(),
        }
    }
}

impl Probe for HostProbe {
    fn sample(&self) -> HardwareSample {
        HostProbe::sample(self)
    }
}

/// `listed` is the sysinfo CPU list length. A zero list uses `parallelism`
/// (`std::thread::available_parallelism`). Both zero stays zero, and the plan
/// still treats the machine as one processor.
pub(crate) fn cores_from_counts(listed: usize, parallelism: Option<usize>) -> u32 {
    let listed = u32::try_from(listed).unwrap_or(u32::MAX);
    if listed > 0 {
        listed
    } else {
        match parallelism {
            Some(count) => u32::try_from(count).unwrap_or(u32::MAX),
            None => 0,
        }
    }
}

pub(crate) fn ram_mib_from_bytes(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}

fn listed_cores_and_ram() -> (usize, u64) {
    let system = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::nothing())
            .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
    );
    (system.cpus().len(), system.total_memory())
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn host_avx2() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn host_avx2() -> bool {
    false
}

#[cfg(target_arch = "aarch64")]
fn host_neon() -> bool {
    std::arch::is_aarch64_feature_detected!("neon")
}

#[cfg(target_arch = "arm")]
fn host_neon() -> bool {
    std::arch::is_arm_feature_detected!("neon")
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
fn host_neon() -> bool {
    false
}

/// Linux NVIDIA probe. Compiled only with the `nvml` feature on Linux.
/// `Nvml::init` returns an error when the library is absent, and that error
/// becomes "no GPU".
#[cfg(all(feature = "nvml", target_os = "linux"))]
pub(crate) fn detect_gpu() -> Option<GpuInfo> {
    let nvml = nvml_wrapper::Nvml::init().ok()?;
    let device = nvml.device_by_index(0).ok()?;
    let name = device.name().unwrap_or_default();
    let memory = device.memory_info().ok()?;
    Some(GpuInfo {
        vendor: GpuVendor::Nvidia,
        name,
        total_mib: memory.total / 1024 / 1024,
        free_mib: memory.free / 1024 / 1024,
    })
}

/// This build has no NVIDIA probe. The profile cannot select GPU compute.
#[cfg(not(all(feature = "nvml", target_os = "linux")))]
pub(crate) fn detect_gpu() -> Option<GpuInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{plan, Backend, GpuChoice};

    #[test]
    fn a_zero_cpu_list_uses_parallelism_and_bytes_become_mib() {
        assert_eq!(cores_from_counts(8, None), 8);
        assert_eq!(cores_from_counts(0, Some(4)), 4);
        assert_eq!(cores_from_counts(0, None), 0);
        assert_eq!(cores_from_counts(1, Some(99)), 1);
        assert_eq!(ram_mib_from_bytes(3 * 1024 * 1024 + 500), 3);
        assert_eq!(ram_mib_from_bytes(1024 * 1024 - 1), 0);
    }

    #[test]
    fn the_compiled_gpu_probe_cannot_force_the_brain_off_cpu() {
        let gpu = detect_gpu();
        if cfg!(all(feature = "nvml", target_os = "linux")) {
            let profile = plan(
                &HardwareSample {
                    cores: 8,
                    ram_mib: 16384,
                    arch: "x86_64".to_string(),
                    avx2: false,
                    neon: false,
                    gpu: gpu.clone(),
                },
                GpuChoice::OptIn,
            );
            assert_eq!(profile.brain, Backend::Cpu);
            assert_eq!(profile.mouth, Backend::Cpu);
            let expect = if gpu.is_some() {
                Backend::Gpu
            } else {
                Backend::Cpu
            };
            assert_eq!(profile.compute, expect);
        } else {
            assert!(gpu.is_none());
        }
    }

    #[test]
    fn the_host_sample_agrees_with_itself_and_stays_on_cpu_by_default() {
        let first = HostProbe.sample();
        let second = HostProbe.sample();
        assert_eq!(first.cores, second.cores);
        assert_eq!(first.ram_mib, second.ram_mib);
        assert_eq!(first.arch, second.arch);
        assert_eq!(first.avx2, second.avx2);
        assert_eq!(first.neon, second.neon);
        assert_eq!(first.gpu.is_some(), second.gpu.is_some());
        assert!(first.cores >= 1);
        assert_eq!(first.arch, std::env::consts::ARCH);
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert!(!first.neon);
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        assert!(!first.avx2);
        #[cfg(not(feature = "nvml"))]
        assert!(first.gpu.is_none());

        let profile = plan(&first, GpuChoice::Cpu);
        assert_eq!(profile.brain, Backend::Cpu);
        assert_eq!(profile.mouth, Backend::Cpu);
        assert_eq!(profile.compute, Backend::Cpu);
        assert_eq!(profile.frame_ms, crate::FRAME_MS);
        assert!(profile.compute_budget_ms <= crate::FRAME_BUDGET_P95_MS);
        assert!(profile.threads.workers >= 1 && profile.threads.workers <= crate::WORKER_CAP);
    }
}
