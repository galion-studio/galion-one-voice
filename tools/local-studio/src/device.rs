// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Device pick for the local studio machine.
//!
//! CPU is the default. `auto` uses CUDA when an NVIDIA GPU is visible.
//! The probe result is passed in so tests do not depend on this host.

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRequest {
    Auto,
    Cpu,
    Cuda,
}

impl Default for DeviceRequest {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Cpu,
    Cuda,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceChoice {
    pub requested: DeviceRequest,
    pub used: DeviceKind,
    /// Value passed to a runtime `--ep` flag: `cpu` or `cuda`.
    pub ep: String,
    pub nvidia: bool,
    pub note: String,
}

pub fn choose(request: DeviceRequest, nvidia: bool) -> Result<DeviceChoice, String> {
    match request {
        DeviceRequest::Cpu => Ok(DeviceChoice {
            requested: request,
            used: DeviceKind::Cpu,
            ep: "cpu".into(),
            nvidia,
            note: "CPU. Explicit cpu request.".into(),
        }),
        DeviceRequest::Cuda if nvidia => Ok(cuda(request)),
        DeviceRequest::Cuda => Err("device cuda was requested and no NVIDIA GPU is visible".into()),
        DeviceRequest::Auto if nvidia => Ok(cuda(request)),
        DeviceRequest::Auto => Ok(DeviceChoice {
            requested: request,
            used: DeviceKind::Cpu,
            ep: "cpu".into(),
            nvidia: false,
            note: "CPU. No NVIDIA GPU is visible.".into(),
        }),
    }
}

fn cuda(requested: DeviceRequest) -> DeviceChoice {
    DeviceChoice {
        requested,
        used: DeviceKind::Cuda,
        ep: "cuda".into(),
        nvidia: true,
        note: "NVIDIA GPU is visible. The clone runtime is started with --ep cuda.".into(),
    }
}

/// True when the machine exposes an NVIDIA device.
///
/// Linux device nodes, a usable `CUDA_VISIBLE_DEVICES` value, or a
/// successful `nvidia-smi` query all count. A missing tool is a CPU machine.
pub fn nvidia_visible() -> bool {
    if std::path::Path::new("/dev/nvidia0").exists()
        || std::path::Path::new("/dev/nvidiactl").exists()
    {
        return true;
    }
    if let Ok(v) = std::env::var("CUDA_VISIBLE_DEVICES") {
        let t = v.trim();
        if !t.is_empty() && t != "-1" {
            return true;
        }
    }
    Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_the_default_and_an_explicit_cpu_wins() {
        let none = choose(DeviceRequest::Auto, false).unwrap();
        assert_eq!(none.used, DeviceKind::Cpu);
        assert_eq!(none.ep, "cpu");
        assert!(!none.nvidia);

        let forced = choose(DeviceRequest::Cpu, true).unwrap();
        assert_eq!(forced.used, DeviceKind::Cpu);
        assert_eq!(forced.ep, "cpu");
        assert!(forced.nvidia);

        let gpu = choose(DeviceRequest::Auto, true).unwrap();
        assert_eq!(gpu.used, DeviceKind::Cuda);
        assert_eq!(gpu.ep, "cuda");

        let err = choose(DeviceRequest::Cuda, false).unwrap_err();
        assert!(err.contains("cuda"));
    }

    #[test]
    fn nvidia_probe_returns() {
        let _ = nvidia_visible();
    }
}
