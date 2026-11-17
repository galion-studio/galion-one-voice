// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! CPU unless a GPU is visible. Same choice as the voice-v2 pipeline
//! (draft PR #194, `tools/symphony/voice-v2/src/device.rs`): CUDA is used
//! when it is visible, ROCm is recorded and the clone stays on CPU because
//! the existing `cbx` binary has no ROCm provider.
//!
//! The Chatterbox worker landing still refuses a non-CPU device. Adapters
//! that talk to that process keep sending `device=cpu`.

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
    /// Value the voice-v2 pipeline would pass to `cbx --ep`.
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

/// `nvidia` and `kfd` are injected so tests do not depend on this machine.
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
        DeviceKind::Cpu => (
            DeviceKind::Cpu,
            "cpu",
            "CPU. Default, and what the existing Chatterbox worker was landed on.",
        ),
        DeviceKind::Cuda => (
            DeviceKind::Cuda,
            "cuda",
            "CUDA is visible. A backend that can use it may. The Chatterbox worker process stays on CPU.",
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
