// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Where a mouth may run.
//!
//! The default build is CPU. It does not stat a device node and it does not
//! read `CUDA_VISIBLE_DEVICES`. The `gpu` feature compiles the probe. A GPU
//! place is returned only when that feature is on and a device is present.
//! This crate does not link a GPU library either way.

/// Compute place for a mouth adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Cpu,
    Gpu,
}

/// `gpu_feature` is `cfg!(feature = "gpu")`. `hardware_present` is the probe.
pub fn select_place(gpu_feature: bool, hardware_present: bool) -> Place {
    if gpu_feature && hardware_present {
        Place::Gpu
    } else {
        Place::Cpu
    }
}

/// Place for this build. CPU unless the `gpu` feature is compiled and the
/// probe sees a device.
pub fn place() -> Place {
    select_place(cfg!(feature = "gpu"), hardware_present())
}

fn hardware_present() -> bool {
    #[cfg(feature = "gpu")]
    {
        probe()
    }
    #[cfg(not(feature = "gpu"))]
    {
        false
    }
}

#[cfg(feature = "gpu")]
fn probe() -> bool {
    if cuda_hidden() {
        return false;
    }
    nvidia_node() || rocm_node() || cuda_listed()
}

#[cfg(feature = "gpu")]
fn cuda_hidden() -> bool {
    match std::env::var("CUDA_VISIBLE_DEVICES") {
        Ok(value) => {
            let trimmed = value.trim();
            trimmed.is_empty() || trimmed == "-1"
        }
        Err(_) => false,
    }
}

#[cfg(feature = "gpu")]
fn cuda_listed() -> bool {
    match std::env::var("CUDA_VISIBLE_DEVICES") {
        Ok(value) => {
            let trimmed = value.trim();
            !trimmed.is_empty() && trimmed != "-1"
        }
        Err(_) => false,
    }
}

#[cfg(all(feature = "gpu", unix))]
fn nvidia_node() -> bool {
    std::path::Path::new("/dev/nvidia0").exists()
}

#[cfg(all(feature = "gpu", windows))]
fn nvidia_node() -> bool {
    std::path::Path::new(r"C:\Windows\System32\nvcuda.dll").exists()
}

#[cfg(all(feature = "gpu", not(any(unix, windows))))]
fn nvidia_node() -> bool {
    false
}

#[cfg(all(feature = "gpu", unix))]
fn rocm_node() -> bool {
    std::path::Path::new("/dev/kfd").exists()
}

#[cfg(all(feature = "gpu", not(unix)))]
fn rocm_node() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_only_when_the_feature_and_the_hardware_are_both_on() {
        assert_eq!(select_place(false, false), Place::Cpu);
        assert_eq!(select_place(false, true), Place::Cpu);
        assert_eq!(select_place(true, false), Place::Cpu);
        assert_eq!(select_place(true, true), Place::Gpu);
    }

    #[test]
    fn this_build_matches_the_gate() {
        assert_eq!(
            place(),
            select_place(cfg!(feature = "gpu"), hardware_present())
        );
    }
}
