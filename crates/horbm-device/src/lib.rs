// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM device profile.
//!
//! At boot, [`boot`] asks a [`Probe`] for one sample and [`plan`] turns that
//! sample into a version-1 profile. Other layers read [`DeviceProfile`] for
//! the thread counts, the frame, the compute budget, and the backends.
//! [`write_profile`] stores the same bytes [`read_profile`] accepts.
//!
//! [`HostProbe`] is the machine. It uses `sysinfo` for logical processors and
//! total RAM, `std::arch` for AVX2 and NEON, and, only with the `nvml`
//! feature on Linux, the `nvml-wrapper` crate for one NVIDIA GPU.
//! [`FixedProbe`] is the stand-in. The same sample always plans the same
//! profile, with no read of the host.
//!
//! The brain and the mouth are CPU. [`GpuChoice::Cpu`] is the default, and a
//! detected GPU is only reported. [`GpuChoice::OptIn`] sets compute to the
//! GPU when the sample has one, and stays on CPU when it does not. This
//! crate does not load a weight, open a socket, or start a thread.
//!
//! # Linux-only
//!
//! The NVIDIA call is `cfg(all(feature = "nvml", target_os = "linux"))`.
//! `Nvml::init` asks the dynamic linker for the library. This crate does not
//! name a filesystem path or a program suffix. On every other target the
//! feature compiles as a no-op and the profile stays on CPU. The workflow
//! that runs this crate is the Linux `gates` job. There is no Windows workflow.
//!
//! The 80 ms frame and the 60 ms compute cap are the locks in `LOCKS.toml`
//! (`clock.frame_ms`, `clock.frame_budget_p95_ms`). The class cuts and the
//! split of that cap across cores are policy, not a measurement.

#![forbid(unsafe_code)]

mod plan;
mod probe;
mod profile;

pub use plan::{
    plan, Backend, DeviceClass, DeviceProfile, GpuChoice, GpuInfo, GpuVendor, HardwareSample,
    ThreadPlan, BOARD_MAX_CORES, BOARD_RAM_MIB, BUDGET_CORE_SPAN, FRAME_BUDGET_P95_MS, FRAME_MS,
    MOUTH_THREADS, PHONE_MAX_CORES, PHONE_RAM_MIB, PROFILE_VERSION, RETRIEVAL_THREADS, WORKER_CAP,
};
pub use probe::{FixedProbe, HostProbe, Probe};
pub use profile::{decode, encode, read_profile, write_profile, ProfileError, MAGIC};

/// Plan `probe` with `choice`. Does not touch the filesystem.
pub fn boot(probe: &impl Probe, choice: GpuChoice) -> DeviceProfile {
    plan(&probe.sample(), choice)
}

/// Plan the live machine. [`GpuChoice::Cpu`] does not use a GPU that is present.
pub fn boot_host(choice: GpuChoice) -> DeviceProfile {
    boot(&HostProbe, choice)
}

/// Plan `probe` and write the profile to `path`.
pub fn boot_to(
    probe: &impl Probe,
    choice: GpuChoice,
    path: &std::path::Path,
) -> Result<DeviceProfile, ProfileError> {
    let profile = boot(probe, choice);
    write_profile(path, &profile)?;
    Ok(profile)
}
