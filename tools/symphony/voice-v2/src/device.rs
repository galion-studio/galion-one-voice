// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Laptop device pick. CPU unless a GPU is visible. An explicit
//! `GALION_DEVICE=cpu` stays on CPU even when a GPU is present.
//!
//! The existing `cbx` binary has `--ep cpu|cuda|coreml|directml`. It has no
//! ROCm flag, so a ROCm box is recorded and the clone still runs on CPU.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Cpu,
    Cuda,
    Rocm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceChoice {
    pub requested: DeviceKind,
    pub used: DeviceKind,
    /// Value passed to `cbx --ep` when that binary is actually spawned.
    pub cbx_ep: String,
    pub gpu_present: bool,
    pub note: String,
}

pub fn detect() -> DeviceChoice {
    let explicit = std::env::var("GALION_DEVICE").ok();
    detect_with(explicit.as_deref())
}

pub fn detect_with(explicit: Option<&str>) -> DeviceChoice {
    let nvidia =
        Path::new("/dev/nvidia0").exists() || Path::new("/dev/nvidiactl").exists() || cuda_env();
    let kfd = Path::new("/dev/kfd").exists();
    choose(explicit, nvidia, kfd)
}

fn cuda_env() -> bool {
    match std::env::var("CUDA_VISIBLE_DEVICES") {
        Ok(v) => {
            let t = v.trim();
            !t.is_empty() && t != "-1"
        }
        Err(_) => false,
    }
}

/// `nvidia` / `kfd` are injected so tests do not depend on this machine.
pub fn choose(explicit: Option<&str>, nvidia: bool, kfd: bool) -> DeviceChoice {
    let gpu_present = nvidia || kfd;
    let requested = match explicit.map(str::trim).map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "cpu" => DeviceKind::Cpu,
        Some(ref s) if s == "cuda" || s == "gpu" => DeviceKind::Cuda,
        Some(ref s) if s == "rocm" => DeviceKind::Rocm,
        _ if nvidia => DeviceKind::Cuda,
        _ if kfd => DeviceKind::Rocm,
        _ => DeviceKind::Cpu,
    };
    let (used, cbx_ep, note) = match requested {
        DeviceKind::Cpu => (DeviceKind::Cpu, "cpu", "CPU. Default, and what the existing Chatterbox layer was measured on."),
        DeviceKind::Cuda => (
            DeviceKind::Cuda,
            "cuda",
            "CUDA is visible. cbx is started with --ep cuda. The synthetic stand-in ignores the device.",
        ),
        DeviceKind::Rocm => (
            DeviceKind::Cpu,
            "cpu",
            "ROCm is visible. cbx has no ROCm execution provider, so the clone stays on CPU.",
        ),
    };
    DeviceChoice {
        requested,
        used,
        cbx_ep: cbx_ep.into(),
        gpu_present,
        note: note.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_the_default_and_an_explicit_cpu_wins() {
        let none = choose(None, false, false);
        assert_eq!(none.used, DeviceKind::Cpu);
        assert_eq!(none.cbx_ep, "cpu");
        assert!(!none.gpu_present);

        let forced = choose(Some("cpu"), true, false);
        assert_eq!(forced.requested, DeviceKind::Cpu);
        assert_eq!(forced.used, DeviceKind::Cpu);

        let gpu = choose(None, true, false);
        assert_eq!(gpu.used, DeviceKind::Cuda);
        assert_eq!(gpu.cbx_ep, "cuda");

        let rocm = choose(None, false, true);
        assert_eq!(rocm.requested, DeviceKind::Rocm);
        assert_eq!(rocm.used, DeviceKind::Cpu);
        assert_eq!(rocm.cbx_ep, "cpu");
    }
}
