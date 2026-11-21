// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Which device `galion-piper-rt` will try. CPU is the default. CUDA is
//! opt-in (`GALION_TTS_DEVICE=cuda` or `--device cuda`) and only when a GPU
//! is visible and a CUDA build of onnxruntime is on disk. Anything else is
//! CPU, with a log line the caller can read. This module does not time
//! inference and does not invent a latency.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Arena cap for the CUDA execution provider. This is a ceiling for an 8 GB
/// laptop GPU, not a measurement of what Piper allocates.
pub const DEFAULT_CUDA_ARENA_BYTES: usize = 2 * 1024 * 1024 * 1024;

pub const ORT_SONAME: &str = "lib/libonnxruntime.so.1.31.1";

const GPU_PACKS: &[&str] = &[
    "onnxruntime-linux-x64-gpu_cuda13-1.31.1",
    "onnxruntime-linux-x64-gpu_cuda12-1.31.1",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Cpu,
    Cuda,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Ask ONNX Runtime for the CUDA execution provider.
    pub try_cuda: bool,
    /// Printed as `[galion-piper-rt] {log}` when set. CPU with no note prints nothing here.
    pub log: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    pub hidden_by_env: bool,
    pub nvidia_smi: bool,
    pub dev_node: bool,
    pub proc_driver: bool,
}

impl Hardware {
    pub fn visible(&self) -> bool {
        if self.hidden_by_env {
            return false;
        }
        self.nvidia_smi || self.dev_node || self.proc_driver
    }

    pub fn reason(&self) -> String {
        if self.hidden_by_env {
            return "CUDA_VISIBLE_DEVICES hides every device".into();
        }
        let mut parts = Vec::new();
        if !self.nvidia_smi {
            parts.push("nvidia-smi did not list a GPU");
        }
        if !self.dev_node {
            parts.push("/dev/nvidia0 is absent");
        }
        if !self.proc_driver {
            parts.push("/proc/driver/nvidia/version is absent");
        }
        parts.join("; ")
    }
}

/// `raw` is the flag or env value. `auto` and empty stay on CPU. `gpu` is
/// the same request as `cuda`. An unknown token is CPU plus a log line.
pub fn parse_request(raw: &str) -> (Request, Option<String>) {
    let token = raw.trim().to_ascii_lowercase();
    match token.as_str() {
        "" | "cpu" | "auto" => (Request::Cpu, None),
        "cuda" | "gpu" => (Request::Cuda, None),
        _ => (
            Request::Cpu,
            Some(format!(
                "unknown GALION_TTS_DEVICE={}; falling back to CPU",
                raw.trim()
            )),
        ),
    }
}

/// Empty and `-1` hide every CUDA device. Unset does not.
pub fn hidden_by_cuda_visible_devices(value: Option<&str>) -> bool {
    match value {
        None => false,
        Some(raw) => {
            let token = raw.trim();
            token.is_empty() || token == "-1"
        }
    }
}

pub fn plan(request: Request, unknown: Option<String>, hardware: &Hardware, gpu_lib: Option<&Path>) -> Plan {
    if let Some(note) = unknown {
        return Plan { try_cuda: false, log: Some(note) };
    }
    match request {
        Request::Cpu => Plan { try_cuda: false, log: None },
        Request::Cuda if !hardware.visible() => Plan {
            try_cuda: false,
            log: Some(format!(
                "GALION_TTS_DEVICE=cuda but CUDA is not available ({}); falling back to CPU",
                hardware.reason()
            )),
        },
        Request::Cuda if gpu_lib.is_none() => Plan {
            try_cuda: false,
            log: Some(
                "GALION_TTS_DEVICE=cuda but the CUDA onnxruntime library is not on disk; falling back to CPU"
                    .into(),
            ),
        },
        Request::Cuda => Plan { try_cuda: true, log: None },
    }
}

pub fn arena_bytes(raw: Option<&str>) -> usize {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return DEFAULT_CUDA_ARENA_BYTES;
    };
    match raw.parse::<usize>() {
        Ok(n) if n > 0 => n,
        _ => DEFAULT_CUDA_ARENA_BYTES,
    }
}

pub fn device_id(raw: Option<&str>) -> i32 {
    raw.and_then(|s| s.trim().parse().ok()).filter(|n| *n >= 0).unwrap_or(0)
}

/// An explicit path that is not a file does not fall through to the search list.
pub fn resolve_cuda_lib(explicit: Option<&Path>, candidates: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_file().then(|| path.to_path_buf());
    }
    candidates.iter().find(|p| p.is_file()).cloned()
}

pub fn cuda_lib_candidates(repo: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(repo) = repo {
        roots.push(repo.join("weights/runtime/ort-cuda"));
    }
    roots.push(PathBuf::from("/mnt/g/galion/ort"));
    let mut out = Vec::new();
    for root in roots {
        for pack in GPU_PACKS {
            out.push(root.join(pack).join(ORT_SONAME));
        }
        out.push(root.join(ORT_SONAME));
    }
    out
}

pub fn probe_hardware() -> Hardware {
    Hardware {
        hidden_by_env: hidden_by_cuda_visible_devices(std::env::var("CUDA_VISIBLE_DEVICES").ok().as_deref()),
        nvidia_smi: nvidia_smi_lists_gpu(),
        dev_node: Path::new("/dev/nvidia0").exists(),
        proc_driver: Path::new("/proc/driver/nvidia/version").is_file(),
    }
}

fn nvidia_smi_lists_gpu() -> bool {
    let mut child = match Command::new("nvidia-smi")
        .arg("-L")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut text = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut text);
                }
                return status.success() && text.lines().any(|line| line.starts_with("GPU "));
            }
            Ok(None) if started.elapsed() > Duration::from_secs(3) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cpu_is_the_default_including_auto() {
        for raw in ["", "cpu", "CPU", " auto "] {
            let (req, note) = parse_request(raw);
            assert_eq!(req, Request::Cpu);
            assert!(note.is_none());
        }
    }

    #[test]
    fn cuda_and_gpu_are_the_only_gpu_requests() {
        assert_eq!(parse_request("cuda").0, Request::Cuda);
        assert_eq!(parse_request("GPU").0, Request::Cuda);
    }

    #[test]
    fn unknown_device_falls_back_with_a_log_line() {
        let (req, note) = parse_request("vulkan");
        assert_eq!(req, Request::Cpu);
        let note = note.unwrap();
        assert!(note.contains("unknown GALION_TTS_DEVICE=vulkan"));
        assert!(note.contains("falling back to CPU"));
    }

    #[test]
    fn hidden_env_beats_a_visible_device_node() {
        assert!(hidden_by_cuda_visible_devices(Some("-1")));
        assert!(hidden_by_cuda_visible_devices(Some("  ")));
        assert!(!hidden_by_cuda_visible_devices(Some("0")));
        assert!(!hidden_by_cuda_visible_devices(None));
        let hw = Hardware { hidden_by_env: true, nvidia_smi: true, dev_node: true, proc_driver: true };
        assert!(!hw.visible());
        let plan = plan(Request::Cuda, None, &hw, Some(Path::new("/tmp/libonnxruntime.so")));
        assert!(!plan.try_cuda);
        assert!(plan.log.unwrap().contains("falling back to CPU"));
    }

    #[test]
    fn cuda_without_a_library_does_not_try_the_provider() {
        let hw = Hardware { hidden_by_env: false, nvidia_smi: true, dev_node: true, proc_driver: true };
        let plan = plan(Request::Cuda, None, &hw, None);
        assert!(!plan.try_cuda);
        assert!(plan.log.unwrap().contains("library is not on disk"));
    }

    #[test]
    fn cuda_with_hardware_and_a_library_is_attempted() {
        let hw = Hardware { hidden_by_env: false, nvidia_smi: false, dev_node: true, proc_driver: false };
        assert!(hw.visible());
        let plan = plan(Request::Cuda, None, &hw, Some(Path::new("/opt/ort/lib/libonnxruntime.so")));
        assert!(plan.try_cuda);
        assert!(plan.log.is_none());
    }

    #[test]
    fn explicit_missing_library_is_not_replaced() {
        let missing = Path::new("/tmp/galion-ort-cuda-missing.so");
        assert!(resolve_cuda_lib(Some(missing), &[PathBuf::from("/tmp/other.so")]).is_none());
    }

    #[test]
    fn explicit_library_file_is_used() {
        let dir = std::env::temp_dir().join(format!("galion-ort-lib-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let lib = dir.join("libonnxruntime.so");
        fs::write(&lib, b"not a real library").unwrap();
        let got = resolve_cuda_lib(Some(&lib), &[]).unwrap();
        assert_eq!(got, lib);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn arena_default_is_the_cap_not_a_measurement() {
        assert_eq!(arena_bytes(None), DEFAULT_CUDA_ARENA_BYTES);
        assert_eq!(arena_bytes(Some("0")), DEFAULT_CUDA_ARENA_BYTES);
        assert_eq!(arena_bytes(Some("4096")), 4096);
        assert_eq!(device_id(None), 0);
        assert_eq!(device_id(Some("1")), 1);
        assert_eq!(device_id(Some("-3")), 0);
    }
}
