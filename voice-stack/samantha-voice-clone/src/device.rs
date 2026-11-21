// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! CUDA when torch says it is available. Otherwise CPU.
//! A missing torch install is CPU, and it is not a CUDA measurement.

use crate::error::TtsError;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputeKind {
    Cpu,
    Cuda,
}

impl ComputeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ComputeKind::Cpu => "cpu",
            ComputeKind::Cuda => "cuda",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputeProbe {
    pub kind: ComputeKind,
    pub note: String,
    pub raw: String,
    pub elapsed_us: u128,
}

pub fn python_bin() -> String {
    if let Ok(value) = std::env::var("GALION_SAMANTHA_PYTHON") {
        let value = value.trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }
    if cfg!(windows) {
        "python".to_string()
    } else {
        "python3".to_string()
    }
}

/// Ask the local interpreter whether torch can see CUDA.
/// This does not download torch and does not load a voice model.
pub fn probe_compute() -> ComputeProbe {
    let started = Instant::now();
    let script = "\
import sys
try:
    import torch
except Exception as exc:
    sys.stdout.write('cpu\\n')
    sys.stderr.write('torch-missing:%s\\n' % type(exc).__name__)
    raise SystemExit(0)
sys.stdout.write('cuda\\n' if torch.cuda.is_available() else 'cpu\\n')
sys.stderr.write('torch\\n')
";
    match Command::new(python_bin()).arg("-c").arg(script).output() {
        Ok(output) => {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let kind = if raw == "cuda" {
                ComputeKind::Cuda
            } else {
                ComputeKind::Cpu
            };
            let note = if err.is_empty() {
                if raw.is_empty() {
                    "probe returned empty stdout".to_string()
                } else {
                    raw.clone()
                }
            } else {
                err
            };
            ComputeProbe {
                kind,
                note,
                raw,
                elapsed_us: started.elapsed().as_micros(),
            }
        }
        Err(err) => ComputeProbe {
            kind: ComputeKind::Cpu,
            note: format!("python probe failed: {err}"),
            raw: String::new(),
            elapsed_us: started.elapsed().as_micros(),
        },
    }
}

pub fn resolve_device(requested: &str, probe: &ComputeProbe) -> Result<String, TtsError> {
    match requested.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(probe.kind.as_str().to_string()),
        "cpu" => Ok("cpu".to_string()),
        "cuda" => {
            if probe.kind == ComputeKind::Cuda {
                Ok("cuda".to_string())
            } else {
                Err(TtsError::new(
                    "device",
                    format!("cuda requested but the probe said cpu ({})", probe.note),
                ))
            }
        }
        other => Err(TtsError::new("device", format!("unknown device {other}"))),
    }
}

pub fn s711_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/s711_worker.py")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_agrees_with_the_process_it_ran() {
        let probe = probe_compute();
        assert!(probe.elapsed_us < 30_000_000, "probe hung: {probe:?}");
        assert!(!probe.note.is_empty(), "{probe:?}");
        match probe.raw.as_str() {
            "cuda" => assert_eq!(probe.kind, ComputeKind::Cuda),
            "cpu" | "" => assert_eq!(probe.kind, ComputeKind::Cpu),
            other => panic!("unexpected probe stdout {other}: {probe:?}"),
        }
    }

    #[test]
    fn cuda_request_fails_closed_when_the_probe_is_cpu() {
        let cpu = ComputeProbe {
            kind: ComputeKind::Cpu,
            note: "torch-missing:ModuleNotFoundError".into(),
            raw: "cpu".into(),
            elapsed_us: 1,
        };
        assert_eq!(resolve_device("auto", &cpu).unwrap(), "cpu");
        assert_eq!(resolve_device("cpu", &cpu).unwrap(), "cpu");
        assert!(resolve_device("cuda", &cpu).is_err());
        let cuda = ComputeProbe {
            kind: ComputeKind::Cuda,
            note: "torch".into(),
            raw: "cuda".into(),
            elapsed_us: 1,
        };
        assert_eq!(resolve_device("cuda", &cuda).unwrap(), "cuda");
        assert_eq!(resolve_device("auto", &cuda).unwrap(), "cuda");
    }
}
