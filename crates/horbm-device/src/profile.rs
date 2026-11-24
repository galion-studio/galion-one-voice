// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Version-1 profile text.
//!
//! One line per field, UTF-8, LF. The first line is [`MAGIC`]. A reader
//! accepts the file only when [`crate::plan`] would have written those same
//! fields for the sample the file describes.

use crate::plan::{
    plan, Backend, DeviceClass, DeviceProfile, GpuChoice, GpuInfo, GpuVendor, HardwareSample,
    ThreadPlan, PROFILE_VERSION,
};
use std::fmt;
use std::fs;
use std::path::Path;

/// First line of a version-1 profile.
pub const MAGIC: &str = "HDP1";

/// Why a profile file was refused or could not be written.
#[derive(Debug, PartialEq, Eq)]
pub enum ProfileError {
    Io(String),
    BadPath,
    BadMagic,
    BadVersion(u16),
    Missing(&'static str),
    Duplicate(&'static str),
    BadValue(&'static str),
    UnknownField,
    BadLine,
    Inconsistent,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Io(msg) => write!(f, "device profile io: {msg}"),
            ProfileError::BadPath => write!(f, "device profile path has no file name"),
            ProfileError::BadMagic => write!(f, "device profile magic is not {MAGIC}"),
            ProfileError::BadVersion(version) => {
                write!(
                    f,
                    "device profile version {version} is not {PROFILE_VERSION}"
                )
            }
            ProfileError::Missing(field) => write!(f, "device profile missing {field}"),
            ProfileError::Duplicate(field) => write!(f, "device profile duplicate {field}"),
            ProfileError::BadValue(field) => write!(f, "device profile bad value for {field}"),
            ProfileError::UnknownField => write!(f, "device profile has an unknown field"),
            ProfileError::BadLine => write!(f, "device profile has a bad line"),
            ProfileError::Inconsistent => {
                write!(f, "device profile does not match the plan for its sample")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

/// Canonical text. The same profile always encodes to the same string.
pub fn encode(profile: &DeviceProfile) -> String {
    let mut lines = Vec::with_capacity(20);
    lines.push(MAGIC.to_string());
    lines.push(format!("version {}", profile.version));
    lines.push(format!("class {}", profile.class.as_str()));
    lines.push(format!("cores {}", profile.cores));
    lines.push(format!("ram_mib {}", profile.ram_mib));
    lines.push(format!("arch {}", profile.arch));
    lines.push(format!("avx2 {}", bit(profile.avx2)));
    lines.push(format!("neon {}", bit(profile.neon)));
    match &profile.gpu {
        None => lines.push("gpu none".to_string()),
        Some(gpu) => {
            lines.push(format!("gpu {}", gpu.vendor.as_str()));
            lines.push(format!("gpu_total_mib {}", gpu.total_mib));
            lines.push(format!("gpu_free_mib {}", gpu.free_mib));
            lines.push(format!("gpu_name {}", gpu.name));
        }
    }
    lines.push(format!("brain {}", profile.brain.as_str()));
    lines.push(format!("mouth {}", profile.mouth.as_str()));
    lines.push(format!("compute {}", profile.compute.as_str()));
    lines.push(format!("retrieval_threads {}", profile.threads.retrieval));
    lines.push(format!("mouth_threads {}", profile.threads.mouth));
    lines.push(format!("worker_threads {}", profile.threads.workers));
    lines.push(format!("frame_ms {}", profile.frame_ms));
    lines.push(format!("compute_budget_ms {}", profile.compute_budget_ms));
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Parse version-1 text. Rejects a file whose fields disagree with [`plan`].
pub fn decode(text: &str) -> Result<DeviceProfile, ProfileError> {
    let mut lines = text.lines();
    let magic = lines.next().ok_or(ProfileError::BadMagic)?;
    if magic != MAGIC {
        return Err(ProfileError::BadMagic);
    }
    let mut raw = Raw::default();
    for line in lines {
        if line.is_empty() {
            return Err(ProfileError::BadLine);
        }
        let (key, value) = split_kv(line)?;
        apply(&mut raw, key, value)?;
    }
    finish(raw)
}

/// Write `profile` to `path` by renaming a sibling temporary file into place.
pub fn write_profile(path: &Path, profile: &DeviceProfile) -> Result<(), ProfileError> {
    let name = path.file_name().ok_or(ProfileError::BadPath)?;
    if name.is_empty() || name == "." || name == ".." {
        return Err(ProfileError::BadPath);
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = parent.join(tmp_name);
    if let Err(err) = fs::write(&tmp, encode(profile)) {
        return Err(ProfileError::Io(err.to_string()));
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(ProfileError::Io(err.to_string()));
    }
    Ok(())
}

/// Read a profile from `path`.
pub fn read_profile(path: &Path) -> Result<DeviceProfile, ProfileError> {
    let text = fs::read_to_string(path).map_err(|err| ProfileError::Io(err.to_string()))?;
    decode(&text)
}

fn bit(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

fn split_kv(line: &str) -> Result<(&str, &str), ProfileError> {
    let (key, value) = line.split_once(' ').ok_or(ProfileError::BadLine)?;
    let value = value.trim();
    if key.is_empty() || value.is_empty() || key.chars().any(char::is_whitespace) {
        return Err(ProfileError::BadLine);
    }
    Ok((key, value))
}

#[derive(Default)]
struct Raw {
    version: Option<u16>,
    class: Option<DeviceClass>,
    cores: Option<u32>,
    ram_mib: Option<u64>,
    arch: Option<String>,
    avx2: Option<bool>,
    neon: Option<bool>,
    gpu_kind: Option<Option<GpuVendor>>,
    gpu_total: Option<u64>,
    gpu_free: Option<u64>,
    gpu_name: Option<String>,
    brain: Option<Backend>,
    mouth: Option<Backend>,
    compute: Option<Backend>,
    retrieval: Option<u32>,
    mouth_threads: Option<u32>,
    workers: Option<u32>,
    frame_ms: Option<u32>,
    budget: Option<u32>,
}

fn apply(raw: &mut Raw, key: &str, value: &str) -> Result<(), ProfileError> {
    match key {
        "version" => set(&mut raw.version, "version", parse_u16(value, "version")?),
        "class" => set(
            &mut raw.class,
            "class",
            DeviceClass::parse(value).ok_or(ProfileError::BadValue("class"))?,
        ),
        "cores" => set(&mut raw.cores, "cores", parse_u32(value, "cores")?),
        "ram_mib" => set(&mut raw.ram_mib, "ram_mib", parse_u64(value, "ram_mib")?),
        "arch" => set(&mut raw.arch, "arch", value.to_string()),
        "avx2" => set(&mut raw.avx2, "avx2", parse_bit(value, "avx2")?),
        "neon" => set(&mut raw.neon, "neon", parse_bit(value, "neon")?),
        "gpu" => set(&mut raw.gpu_kind, "gpu", parse_gpu_kind(value)?),
        "gpu_total_mib" => set(
            &mut raw.gpu_total,
            "gpu_total_mib",
            parse_u64(value, "gpu_total_mib")?,
        ),
        "gpu_free_mib" => set(
            &mut raw.gpu_free,
            "gpu_free_mib",
            parse_u64(value, "gpu_free_mib")?,
        ),
        "gpu_name" => set(&mut raw.gpu_name, "gpu_name", value.to_string()),
        "brain" => set(
            &mut raw.brain,
            "brain",
            Backend::parse(value).ok_or(ProfileError::BadValue("brain"))?,
        ),
        "mouth" => set(
            &mut raw.mouth,
            "mouth",
            Backend::parse(value).ok_or(ProfileError::BadValue("mouth"))?,
        ),
        "compute" => set(
            &mut raw.compute,
            "compute",
            Backend::parse(value).ok_or(ProfileError::BadValue("compute"))?,
        ),
        "retrieval_threads" => set(
            &mut raw.retrieval,
            "retrieval_threads",
            parse_u32(value, "retrieval_threads")?,
        ),
        "mouth_threads" => set(
            &mut raw.mouth_threads,
            "mouth_threads",
            parse_u32(value, "mouth_threads")?,
        ),
        "worker_threads" => set(
            &mut raw.workers,
            "worker_threads",
            parse_u32(value, "worker_threads")?,
        ),
        "frame_ms" => set(&mut raw.frame_ms, "frame_ms", parse_u32(value, "frame_ms")?),
        "compute_budget_ms" => set(
            &mut raw.budget,
            "compute_budget_ms",
            parse_u32(value, "compute_budget_ms")?,
        ),
        _ => Err(ProfileError::UnknownField),
    }
}

fn set<T>(slot: &mut Option<T>, field: &'static str, value: T) -> Result<(), ProfileError> {
    if slot.is_some() {
        return Err(ProfileError::Duplicate(field));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_gpu_kind(value: &str) -> Result<Option<GpuVendor>, ProfileError> {
    match value {
        "none" => Ok(None),
        other => GpuVendor::parse(other)
            .map(Some)
            .ok_or(ProfileError::BadValue("gpu")),
    }
}

fn parse_bit(value: &str, field: &'static str) -> Result<bool, ProfileError> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(ProfileError::BadValue(field)),
    }
}

fn parse_u16(value: &str, field: &'static str) -> Result<u16, ProfileError> {
    value.parse().map_err(|_| ProfileError::BadValue(field))
}

fn parse_u32(value: &str, field: &'static str) -> Result<u32, ProfileError> {
    value.parse().map_err(|_| ProfileError::BadValue(field))
}

fn parse_u64(value: &str, field: &'static str) -> Result<u64, ProfileError> {
    value.parse().map_err(|_| ProfileError::BadValue(field))
}

fn need<T>(slot: Option<T>, field: &'static str) -> Result<T, ProfileError> {
    slot.ok_or(ProfileError::Missing(field))
}

fn finish(raw: Raw) -> Result<DeviceProfile, ProfileError> {
    let version = need(raw.version, "version")?;
    if version != PROFILE_VERSION {
        return Err(ProfileError::BadVersion(version));
    }
    let gpu = match need(raw.gpu_kind, "gpu")? {
        None => {
            if raw.gpu_total.is_some() || raw.gpu_free.is_some() || raw.gpu_name.is_some() {
                return Err(ProfileError::BadValue("gpu"));
            }
            None
        }
        Some(vendor) => Some(GpuInfo {
            vendor,
            name: need(raw.gpu_name, "gpu_name")?,
            total_mib: need(raw.gpu_total, "gpu_total_mib")?,
            free_mib: need(raw.gpu_free, "gpu_free_mib")?,
        }),
    };
    let cores = need(raw.cores, "cores")?;
    let ram_mib = need(raw.ram_mib, "ram_mib")?;
    let arch = need(raw.arch, "arch")?;
    let avx2 = need(raw.avx2, "avx2")?;
    let neon = need(raw.neon, "neon")?;
    let compute = need(raw.compute, "compute")?;
    let claimed = DeviceProfile {
        version,
        class: need(raw.class, "class")?,
        cores,
        ram_mib,
        arch: arch.clone(),
        avx2,
        neon,
        gpu: gpu.clone(),
        brain: need(raw.brain, "brain")?,
        mouth: need(raw.mouth, "mouth")?,
        compute,
        threads: ThreadPlan {
            retrieval: need(raw.retrieval, "retrieval_threads")?,
            mouth: need(raw.mouth_threads, "mouth_threads")?,
            workers: need(raw.workers, "worker_threads")?,
        },
        frame_ms: need(raw.frame_ms, "frame_ms")?,
        compute_budget_ms: need(raw.budget, "compute_budget_ms")?,
    };
    let choice = match (compute, gpu.is_some()) {
        (Backend::Gpu, true) => GpuChoice::OptIn,
        (Backend::Gpu, false) => return Err(ProfileError::Inconsistent),
        (Backend::Cpu, _) => GpuChoice::Cpu,
    };
    let planned = plan(
        &HardwareSample {
            cores,
            ram_mib,
            arch,
            avx2,
            neon,
            gpu,
        },
        choice,
    );
    if planned != claimed {
        return Err(ProfileError::Inconsistent);
    }
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{DeviceClass, GpuChoice, GpuVendor};

    fn sample(cores: u32, ram_mib: u64, gpu: bool) -> HardwareSample {
        HardwareSample {
            cores,
            ram_mib,
            arch: "aarch64".to_string(),
            avx2: false,
            neon: true,
            gpu: gpu.then(|| GpuInfo {
                vendor: GpuVendor::Nvidia,
                name: "Example Card".to_string(),
                total_mib: 4096,
                free_mib: 1024,
            }),
        }
    }

    #[test]
    fn encode_is_stable_and_round_trips() {
        let cpu = plan(&sample(8, 16384, false), GpuChoice::Cpu);
        let text = encode(&cpu);
        assert_eq!(encode(&cpu), text);
        assert_eq!(decode(&text).unwrap(), cpu);
        assert!(text.starts_with("HDP1\n"));
        assert!(text.ends_with('\n'));
        assert!(!text.contains('.'));
        assert!(!text.contains("http"));
        assert!(!text.contains(".wav"));

        let gpu = plan(&sample(8, 16384, true), GpuChoice::OptIn);
        let gpu_text = encode(&gpu);
        assert_eq!(decode(&gpu_text).unwrap(), gpu);
        assert!(gpu_text.contains("gpu nvidia\n"));
        assert!(gpu_text.contains("gpu_name Example Card\n"));
        assert!(gpu_text.contains("compute gpu\n"));
        assert!(gpu_text.contains("brain cpu\n"));
        assert!(gpu_text.contains("mouth cpu\n"));
        assert!(!gpu_text.contains('.'));
    }

    #[test]
    fn a_bad_file_is_rejected() {
        let text = encode(&plan(&sample(2, 512, false), GpuChoice::Cpu));
        assert_eq!(decode("").unwrap_err(), ProfileError::BadMagic);
        assert_eq!(decode("NOPE\n").unwrap_err(), ProfileError::BadMagic);
        assert_eq!(
            decode(&text.replace("version 1", "version 2")).unwrap_err(),
            ProfileError::BadVersion(2)
        );
        assert!(matches!(
            decode(&text.replace("frame_ms 80", "frame_ms 40")),
            Err(ProfileError::Inconsistent)
        ));
        assert_eq!(
            decode(&format!("{text}extra 1\n")).unwrap_err(),
            ProfileError::UnknownField
        );
        assert_eq!(
            decode(&format!("{text}cores 1\n")).unwrap_err(),
            ProfileError::Duplicate("cores")
        );
        assert!(matches!(
            decode(&text.replace("worker_threads 1", "worker_threads 4")),
            Err(ProfileError::Inconsistent)
        ));
        let stripped = text
            .lines()
            .filter(|line| !line.starts_with("class "))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            decode(&stripped).unwrap_err(),
            ProfileError::Missing("class")
        );
        assert_eq!(
            decode(&text.replace("class phone", "class server")).unwrap_err(),
            ProfileError::BadValue("class")
        );
        let gpu = encode(&plan(&sample(8, 16384, true), GpuChoice::OptIn));
        let forced = gpu.replace("compute gpu", "compute cpu");
        let read = decode(&forced).unwrap();
        assert_eq!(read.compute, Backend::Cpu);
        assert_eq!(read.class, DeviceClass::Laptop);
        assert!(read.gpu.is_some());
    }

    #[test]
    fn a_path_with_no_file_name_is_rejected() {
        let profile = plan(&sample(4, 4096, false), GpuChoice::Cpu);
        assert_eq!(
            write_profile(Path::new(""), &profile).unwrap_err(),
            ProfileError::BadPath
        );
    }
}
