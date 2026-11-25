// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Boot writes the profile the stand-in planned. The frame numbers match the locks.

use horbm_device::{
    boot, boot_to, read_profile, Backend, DeviceClass, DeviceProfile, FixedProbe, GpuChoice,
    GpuInfo, GpuVendor, HardwareSample, FRAME_BUDGET_P95_MS, FRAME_MS,
};
use std::fs;
use std::path::PathBuf;

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

fn card() -> GpuInfo {
    GpuInfo {
        vendor: GpuVendor::Nvidia,
        name: "Example".to_string(),
        total_mib: 24564,
        free_mib: 23564,
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("horbm-device-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_stand_in_boot_is_the_same_profile_twice_and_ignores_the_host() {
    let probe = FixedProbe {
        sample: sample(2, 1024, None),
    };
    let first = boot(&probe, GpuChoice::OptIn);
    let second = boot(&probe, GpuChoice::OptIn);
    assert_eq!(first, second);
    assert_eq!(first.class, DeviceClass::Phone);
    assert_eq!(first.brain, Backend::Cpu);
    assert_eq!(first.mouth, Backend::Cpu);
    assert_eq!(first.compute, Backend::Cpu);
    assert_eq!(first.frame_ms, 80);
    assert_eq!(first.threads.retrieval, 1);
    assert_eq!(first.threads.mouth, 1);
    assert_eq!(first.threads.workers, 1);
}

#[test]
fn a_fixture_with_a_gpu_and_a_fixture_without_both_keep_the_cpu_brain() {
    let bare = boot(
        &FixedProbe {
            sample: sample(8, 16384, None),
        },
        GpuChoice::Cpu,
    );
    let with = boot(
        &FixedProbe {
            sample: sample(8, 16384, Some(card())),
        },
        GpuChoice::Cpu,
    );
    assert_eq!(bare.brain, Backend::Cpu);
    assert_eq!(with.brain, Backend::Cpu);
    assert_eq!(bare.mouth, Backend::Cpu);
    assert_eq!(with.mouth, Backend::Cpu);
    assert_eq!(bare.compute, Backend::Cpu);
    assert_eq!(with.compute, Backend::Cpu);
    assert!(bare.gpu.is_none());
    assert!(with.gpu.is_some());

    let opted = boot(
        &FixedProbe {
            sample: sample(8, 16384, Some(card())),
        },
        GpuChoice::OptIn,
    );
    assert_eq!(opted.compute, Backend::Gpu);
    assert_eq!(opted.brain, Backend::Cpu);
    assert_eq!(opted.mouth, Backend::Cpu);
    let missed = boot(
        &FixedProbe {
            sample: sample(8, 16384, None),
        },
        GpuChoice::OptIn,
    );
    assert_eq!(missed.compute, Backend::Cpu);
}

#[test]
fn boot_to_writes_the_profile_read_returns() {
    let dir = TempDir::new("boot");
    let path = dir.0.join("device-profile.hdp");
    let probe = FixedProbe {
        sample: sample(4, 4096, Some(card())),
    };
    let written = boot_to(&probe, GpuChoice::Cpu, &path).unwrap();
    let read = read_profile(&path).unwrap();
    assert_eq!(written, read);
    assert_eq!(read.class, DeviceClass::Board);
    assert_eq!(read.compute, Backend::Cpu);
    assert_eq!(
        read.gpu.as_ref().map(|gpu| gpu.name.as_str()),
        Some("Example")
    );
    let again = boot_to(&probe, GpuChoice::OptIn, &path).unwrap();
    assert_eq!(read_profile(&path).unwrap(), again);
    assert_eq!(again.compute, Backend::Gpu);
    assert_eq!(again.brain, Backend::Cpu);
}

#[test]
fn the_frame_matches_the_lock_file() {
    let locks = include_str!("../../../LOCKS.toml");
    assert_eq!(lock_u32(locks, "frame_ms"), FRAME_MS);
    assert_eq!(lock_u32(locks, "frame_budget_p95_ms"), FRAME_BUDGET_P95_MS);
    let profile: DeviceProfile = boot(
        &FixedProbe {
            sample: sample(8, 16384, None),
        },
        GpuChoice::Cpu,
    );
    assert_eq!(profile.frame_ms, lock_u32(locks, "frame_ms"));
    assert!(profile.compute_budget_ms <= lock_u32(locks, "frame_budget_p95_ms"));
}

fn lock_u32(text: &str, key: &str) -> u32 {
    text.lines()
        .find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(key)?.trim();
            let rest = rest.strip_prefix('=')?.trim();
            rest.split_whitespace().next()?.parse().ok()
        })
        .unwrap_or_else(|| panic!("missing {key}"))
}
