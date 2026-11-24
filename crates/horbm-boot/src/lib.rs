// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Voice boot profile for HORBM phase 8.
//!
//! Open Mind `galion-boot` writes `hardware.json`. This crate reads that
//! file. It does not run the probe, does not call `nvidia-smi`, and does not
//! link an Open Mind crate.
//!
//! The probe's `llm` value (`api`, `local-gpu`, `lan`, `cpu-light`) is stored
//! on [`VoiceBoot::probe_llm`] and is not the brain. The brain is the name
//! [`BRAIN_NAME`] (`horbm`). The mouth device is [`MOUTH_DEVICE`] (`cpu`).
//! A GPU object is reported on [`VoiceBoot::gpu`] and does not change either
//! choice. That selection is the same whether or not the unmerged phone wire
//! `GALION_LLM=horbm` has landed: callers compare the name.
//! `LOCKS.toml` `[boot]` records the same two names. Consent in that file
//! is a separate section and is not read here.
//!
//! The degrade `mouth` boolean is a RAM threshold in the probe
//! (`ram_total_mib >= 8192`). It is stored on [`VoiceBoot::probe_mouth`].
//! It is not [`VoiceBoot::mouth_device`].
//!
//! `machine.json` is a different document: there `gpu` is a string. Passing
//! that file returns [`BootError::MachineDocument`].

#![forbid(unsafe_code)]

use serde_json::{Map, Value};
use std::fmt;
use std::path::Path;

/// Brain the voice selects. A later phone wire can pass this string through.
/// On current `main`, `LlmMode::from_choice` does not accept it yet.
pub const BRAIN_NAME: &str = "horbm";

/// Mouth device. A reported GPU does not replace this.
pub const MOUTH_DEVICE: &str = "cpu";

/// GPU object from `hardware.json`, or an absent card (`gpu` null or missing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuFlag {
    pub present: bool,
    pub name: String,
    pub total_mib: u64,
    pub used_mib: u64,
    pub free_mib: u64,
}

impl GpuFlag {
    fn absent() -> Self {
        Self {
            present: false,
            name: String::new(),
            total_mib: 0,
            used_mib: 0,
            free_mib: 0,
        }
    }
}

/// What the voice does with one probe file.
///
/// `brain` and `mouth_device` are the constants above. The `probe_*` fields
/// are a report of the file. They do not choose the brain or the mouth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceBoot {
    pub brain: &'static str,
    pub mouth_device: &'static str,
    pub gpu: GpuFlag,
    pub probe_llm: String,
    pub probe_rule: String,
    pub probe_line: String,
    pub probe_mouth: bool,
    pub probe_weights: bool,
}

impl VoiceBoot {
    /// Parse a `hardware.json` body and select the CPU brain.
    pub fn select(bytes: &[u8]) -> Result<Self, BootError> {
        let bytes = strip_bom(bytes);
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| BootError::Json(e.to_string()))?;
        let map = value.as_object().ok_or(BootError::NotAnObject)?;
        let gpu = gpu_flag(map.get("gpu"))?;
        Ok(Self {
            brain: BRAIN_NAME,
            mouth_device: MOUTH_DEVICE,
            gpu,
            probe_llm: opt_string(map, "llm")?,
            probe_rule: opt_string(map, "rule")?,
            probe_line: opt_string(map, "line")?,
            probe_mouth: opt_bool(map, "mouth")?,
            probe_weights: opt_bool(map, "weights")?,
        })
    }

    /// Read `path` and [`select`](Self::select) it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BootError> {
        let path = path.as_ref();
        let bytes =
            std::fs::read(path).map_err(|e| BootError::Io(format!("{}: {e}", path.display())))?;
        Self::select(&bytes)
    }

    /// True when the probe file carried a GPU object, including a DRM card
    /// whose free memory is zero.
    pub fn gpu_reported(&self) -> bool {
        self.gpu.present
    }

    /// True when the selected brain is [`BRAIN_NAME`] and the mouth is [`MOUTH_DEVICE`].
    pub fn selects_cpu_brain(&self) -> bool {
        self.brain == BRAIN_NAME && self.mouth_device == MOUTH_DEVICE
    }
}

impl fmt::Display for VoiceBoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "brain={} mouth={} gpu={}",
            self.brain,
            self.mouth_device,
            if self.gpu.present { "true" } else { "false" }
        )?;
        if self.gpu.present {
            write!(
                f,
                " name={:?} total_mib={} used_mib={} free_mib={}",
                self.gpu.name, self.gpu.total_mib, self.gpu.used_mib, self.gpu.free_mib
            )?;
        }
        write!(
            f,
            " probe_llm={} probe_rule={} probe_mouth={} probe_weights={}",
            self.probe_llm, self.probe_rule, self.probe_mouth, self.probe_weights
        )
    }
}

/// Why a file was not a voice boot profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootError {
    Io(String),
    Json(String),
    NotAnObject,
    /// `gpu` is a string. That is `machine.json`, not `hardware.json`.
    MachineDocument,
    GpuShape(String),
    Field {
        name: &'static str,
        detail: String,
    },
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BootError::Io(m) => write!(f, "{m}"),
            BootError::Json(m) => write!(f, "hardware.json: {m}"),
            BootError::NotAnObject => write!(f, "hardware.json must be a JSON object"),
            BootError::MachineDocument => write!(
                f,
                "gpu is a string. hardware.json stores gpu as null or an object. This value is the machine.json shape"
            ),
            BootError::GpuShape(m) => write!(f, "gpu object: {m}"),
            BootError::Field { name, detail } => write!(f, "{name}: {detail}"),
        }
    }
}

impl std::error::Error for BootError {}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    }
}

fn gpu_flag(value: Option<&Value>) -> Result<GpuFlag, BootError> {
    match value {
        None | Some(Value::Null) => Ok(GpuFlag::absent()),
        Some(Value::String(_)) => Err(BootError::MachineDocument),
        Some(Value::Object(map)) => parse_gpu_object(map),
        Some(_) => Err(BootError::GpuShape("expected null or an object".into())),
    }
}

fn parse_gpu_object(map: &Map<String, Value>) -> Result<GpuFlag, BootError> {
    let name = match map.get("name") {
        Some(Value::String(name)) => name.clone(),
        _ => return Err(BootError::GpuShape("name must be a string".into())),
    };
    Ok(GpuFlag {
        present: true,
        name,
        total_mib: u64_field(map, "total_mib")?,
        used_mib: u64_field(map, "used_mib")?,
        free_mib: u64_field(map, "free_mib")?,
    })
}

fn u64_field(map: &Map<String, Value>, key: &'static str) -> Result<u64, BootError> {
    match map.get(key) {
        Some(Value::Number(n)) => n
            .as_u64()
            .ok_or_else(|| BootError::GpuShape(format!("{key} is not a whole number"))),
        _ => Err(BootError::GpuShape(format!("missing {key}"))),
    }
}

fn opt_string(map: &Map<String, Value>, key: &'static str) -> Result<String, BootError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(BootError::Field {
            name: key,
            detail: "expected a string".into(),
        }),
    }
}

fn opt_bool(map: &Map<String, Value>, key: &'static str) -> Result<bool, BootError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(v)) => Ok(*v),
        Some(_) => Err(BootError::Field {
            name: key,
            detail: "expected a bool".into(),
        }),
    }
}
