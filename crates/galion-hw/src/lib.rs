// G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
// Developed and distributed by Galion Labs
// Perpetual Open MIT License
// Copyright © 2026 Galion Labs. All rights reserved.
//! Hardware detect + device profile. Assembled, not invented: every fact comes
//! from an existing brick and this file is only the glue between them.
//!
//! | fact | brick |
//! |---|---|
//! | OS, arch, CPU brand, cores/threads, RAM, cgroup RAM cap | `sysinfo` 0.36.1 (MIT) |
//! | NVIDIA name, VRAM total/free, driver, CUDA, compute cap | `nvml-wrapper` 0.11.0 (MIT OR Apache-2.0), libnvidia-ml loaded at runtime |
//! | every other GPU (AMD, Intel, Apple, Mali, Adreno, VideoCore) + backends | `wgpu` 23.0.1 adapter enumeration (MIT OR Apache-2.0) |
//! | SIMD level (AVX-512 / AVX2 / NEON / scalar) | `galion-tensor::features::detected` (existing Galion code) |
//! | NPU device nodes, WSL, ROCm/OpenVINO install dirs | path checks (same style as obsidian-open-mind `galion-boot/src/hardware.rs` `drm_card`) |
//!
//! Every brick is optional at run time: a missing driver, loader or library
//! leaves that part empty and the profile falls back to CPU. `GALION_ACCEL=cpu`
//! forces the CPU profile even when a GPU is found.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCHEMA: u32 = 1;
const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Gpu {
    pub name: String,
    pub vendor: String,
    pub vendor_id: u32,
    /// discrete | integrated | virtual | software | other
    pub kind: String,
    /// Which bricks saw it: "nvml", "wgpu".
    pub seen_by: Vec<String>,
    /// Graphics APIs wgpu opened it with: vulkan, metal, dx12, gl.
    pub apis: Vec<String>,
    pub vram_total_mib: Option<u64>,
    pub vram_free_mib: Option<u64>,
    pub driver: Option<String>,
    pub cuda_driver: Option<String>,
    pub compute_capability: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Facts {
    pub os: String,
    pub os_version: String,
    pub kernel: String,
    pub arch: String,
    pub wsl: bool,
    pub cpu_brand: String,
    pub physical_cores: usize,
    pub logical_threads: usize,
    pub simd: String,
    pub ram_total_mib: u64,
    pub ram_available_mib: u64,
    /// Container/cgroup memory cap when lower than the host RAM.
    pub ram_limit_mib: Option<u64>,
    pub gpus: Vec<Gpu>,
    /// NPU / AI accelerator device nodes found (Intel/AMD/Qualcomm accel, Coral, Hailo, Apple ANE).
    pub npus: Vec<String>,
    /// Vendor runtimes found installed: rocm, openvino.
    pub runtimes: Vec<String>,
    /// Bricks that failed or were absent, with the reason (never fatal).
    pub probe_notes: Vec<String>,
}

// ------------------------------------------------------------------ detect

/// Run every brick and collect the facts. Never panics on missing hardware.
pub fn detect() -> Facts {
    let mut f = Facts::default();
    cpu_ram(&mut f);
    f.simd = galion_tensor::features::detected().name().to_string();
    #[cfg(feature = "nvidia")]
    nvidia(&mut f);
    #[cfg(not(feature = "nvidia"))]
    f.probe_notes.push("nvml: feature off".into());
    #[cfg(feature = "gpu-enum")]
    wgpu_adapters(&mut f);
    #[cfg(not(feature = "gpu-enum"))]
    f.probe_notes.push("wgpu: feature off".into());
    nodes(&mut f, Path::new("/"));
    f
}

fn cpu_ram(f: &mut Facts) {
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    f.os = std::env::consts::OS.into();
    f.os_version = System::long_os_version().unwrap_or_default();
    f.kernel = System::kernel_version().unwrap_or_default();
    f.arch = std::env::consts::ARCH.into();
    f.cpu_brand = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    f.logical_threads = sys.cpus().len().max(1);
    f.physical_cores = System::physical_core_count().unwrap_or(f.logical_threads);
    f.ram_total_mib = sys.total_memory() / MIB;
    f.ram_available_mib = sys.available_memory() / MIB;
    if let Some(cg) = sys.cgroup_limits() {
        let cap = cg.total_memory / MIB;
        if cap > 0 && cap < f.ram_total_mib {
            f.ram_limit_mib = Some(cap);
        }
    }
    // sysinfo reads the container-root cgroup only; a nested cgroup v2 cap
    // (systemd scope/service, k8s pod) lives on this process's own path.
    if let Some(cap) = own_cgroup_cap_mib(Path::new("/")) {
        if cap < f.ram_limit_mib.unwrap_or(f.ram_total_mib) {
            f.ram_limit_mib = Some(cap);
        }
    }
    f.wsl = f.kernel.to_ascii_lowercase().contains("microsoft") || Path::new("/dev/dxg").exists();
}

/// Smallest cgroup v2 `memory.max` on this process's path and its parents.
pub fn own_cgroup_cap_mib(root: &Path) -> Option<u64> {
    let cg = std::fs::read_to_string(root.join("proc/self/cgroup")).ok()?;
    let rel = cg.lines().find_map(|l| l.strip_prefix("0::"))?.trim().trim_start_matches('/');
    let mut dir = root.join("sys/fs/cgroup").join(rel);
    let mut best: Option<u64> = None;
    loop {
        if let Ok(v) = std::fs::read_to_string(dir.join("memory.max")) {
            if let Ok(b) = v.trim().parse::<u64>() {
                best = Some(best.map_or(b, |x: u64| x.min(b)));
            }
        }
        if dir.ends_with("sys/fs/cgroup") || !dir.pop() {
            break;
        }
    }
    best.map(|b| b / MIB)
}

#[cfg(feature = "nvidia")]
fn nvidia(f: &mut Facts) {
    use nvml_wrapper::Nvml;
    // Default soname first, then the versioned one (WSL ships only .so.1 in /usr/lib/wsl/lib).
    let nvml = Nvml::init().or_else(|e| {
        if cfg!(target_os = "linux") {
            Nvml::builder().lib_path(std::ffi::OsStr::new("libnvidia-ml.so.1")).init()
        } else {
            Err(e)
        }
    });
    let nvml = match nvml {
        Ok(n) => n,
        Err(e) => return f.probe_notes.push(format!("nvml: {e}")),
    };
    let driver = nvml.sys_driver_version().ok();
    let cuda = nvml.sys_cuda_driver_version().ok().map(|v| format!("{}.{}", v / 1000, (v % 1000) / 10));
    for i in 0..nvml.device_count().unwrap_or(0) {
        let Ok(d) = nvml.device_by_index(i) else { continue };
        let mem = d.memory_info().ok();
        f.gpus.push(Gpu {
            name: d.name().unwrap_or_else(|_| "NVIDIA GPU".into()),
            vendor: "nvidia".into(),
            vendor_id: 0x10de,
            kind: "discrete".into(),
            seen_by: vec!["nvml".into()],
            apis: vec![],
            vram_total_mib: mem.as_ref().map(|m| m.total / MIB),
            vram_free_mib: mem.as_ref().map(|m| m.free / MIB),
            driver: driver.clone(),
            cuda_driver: cuda.clone(),
            compute_capability: d.cuda_compute_capability().ok().map(|c| format!("{}.{}", c.major, c.minor)),
        });
    }
}

#[cfg(feature = "gpu-enum")]
fn wgpu_adapters(f: &mut Facts) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    if adapters.is_empty() {
        f.probe_notes.push("wgpu: no adapters".into());
    }
    for a in adapters {
        let info = a.get_info();
        let api = format!("{:?}", info.backend).to_ascii_lowercase();
        let kind = match info.device_type {
            wgpu::DeviceType::DiscreteGpu => "discrete",
            wgpu::DeviceType::IntegratedGpu => "integrated",
            wgpu::DeviceType::VirtualGpu => "virtual",
            wgpu::DeviceType::Cpu => "software",
            wgpu::DeviceType::Other => "other",
        };
        merge_adapter(f, &info.name, info.vendor, kind, &api, &info.driver_info);
    }
}

/// Fold one wgpu adapter into the GPU list: same card seen over Vulkan, DX12
/// and GL (or already by NVML) becomes one entry with several `apis`.
pub fn merge_adapter(f: &mut Facts, name: &str, vendor_id: u32, kind: &str, api: &str, driver: &str) {
    let key = norm(name);
    let hit = f.gpus.iter_mut().find(|g| {
        (g.vendor_id == vendor_id || vendor_id == 0) && (norm(&g.name) == key || norm(&g.name).contains(&key) || key.contains(&norm(&g.name)))
    });
    match hit {
        Some(g) => {
            if !g.seen_by.iter().any(|s| s == "wgpu") {
                g.seen_by.push("wgpu".into());
            }
            if !g.apis.iter().any(|a| a == api) {
                g.apis.push(api.into());
            }
            if g.kind == "other" && kind != "other" {
                g.kind = kind.into();
            }
        }
        None => f.gpus.push(Gpu {
            name: name.trim().into(),
            vendor: vendor_name(vendor_id).into(),
            vendor_id,
            kind: kind.into(),
            seen_by: vec!["wgpu".into()],
            apis: vec![api.into()],
            driver: (!driver.trim().is_empty()).then(|| driver.trim().to_string()),
            ..Default::default()
        }),
    }
}

fn norm(s: &str) -> String {
    s.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// PCI vendor ids (pci-ids.ucw.cz) for the GPUs Galion can land on.
pub fn vendor_name(id: u32) -> &'static str {
    match id {
        0x10de => "nvidia",
        0x1002 | 0x1022 => "amd",
        0x8086 => "intel",
        0x106b => "apple",
        0x13b5 => "arm",
        0x5143 => "qualcomm",
        0x14e4 => "broadcom",
        0x1010 => "imagination",
        0x10005 => "mesa",
        _ => "other",
    }
}

/// Device-node and install-dir checks under `root` (testable with a fake root).
pub fn nodes(f: &mut Facts, root: &Path) {
    // Linux accel subsystem (Intel NPU, AMD XDNA, Qualcomm AI100), Coral, Hailo.
    if let Ok(rd) = std::fs::read_dir(root.join("dev/accel")) {
        for e in rd.flatten() {
            f.npus.push(format!("/dev/accel/{}", e.file_name().to_string_lossy()));
        }
    }
    for n in ["dev/apex_0", "dev/hailo0"] {
        if root.join(n).exists() {
            f.npus.push(format!("/{n}"));
        }
    }
    if f.os == "macos" && f.arch == "aarch64" {
        f.npus.push("apple-neural-engine".into());
    }
    // amdgpu (and xe/i915 discrete) publish VRAM in sysfs; fill cards wgpu saw without a number.
    if let Ok(rd) = std::fs::read_dir(root.join("sys/class/drm")) {
        for e in rd.flatten() {
            let dev = e.path().join("device");
            let read = |n: &str| std::fs::read_to_string(dev.join(n)).ok().map(|s| s.trim().to_string());
            let (Some(vendor), Some(total)) = (read("vendor"), read("mem_info_vram_total")) else { continue };
            let vid = u32::from_str_radix(vendor.trim_start_matches("0x"), 16).unwrap_or(0);
            let total: u64 = total.parse().unwrap_or(0) / MIB;
            let used: u64 = read("mem_info_vram_used").and_then(|s| s.parse().ok()).unwrap_or(0) / MIB;
            if let Some(g) = f.gpus.iter_mut().find(|g| g.vendor_id == vid && g.vram_total_mib.is_none()) {
                g.vram_total_mib = Some(total);
                g.vram_free_mib = Some(total.saturating_sub(used));
            }
        }
    }
    if root.join("opt/rocm").is_dir() {
        f.runtimes.push("rocm".into());
    }
    if root.join("opt/intel/openvino").is_dir() || std::env::var_os("INTEL_OPENVINO_DIR").is_some() {
        f.runtimes.push("openvino".into());
    }
}

// ------------------------------------------------------------------ profile

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Role {
    pub backend: String,
    /// Size class of the model to load: tiny | small | medium | large | remote.
    pub size: String,
    pub threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Threads {
    pub total: usize,
    /// Core 0 stays with the bus / heartbeat / audio I/O (galion-cores v3 map).
    pub bus: usize,
    pub compute: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub schema: u32,
    pub policy: String,
    pub tier: String,
    /// Backends this device can run, best first; always ends with "cpu".
    pub backends: Vec<String>,
    pub primary: String,
    pub accel_gpu: Option<String>,
    pub threads: Threads,
    pub asr: Role,
    pub tts: Role,
    pub emotion: Role,
    pub reasoning: Role,
    /// Execution-provider names for out-of-process ORT runtimes (galion-piper-rt).
    pub ort_providers: Vec<String>,
    pub facts: Facts,
}

fn has_api(g: &Gpu, api: &str) -> bool {
    g.apis.iter().any(|a| a == api)
}

/// Backends a GPU can serve, best first (thin vendor table).
pub fn gpu_backends(g: &Gpu, f: &Facts) -> Vec<&'static str> {
    let mut v = Vec::new();
    match g.vendor.as_str() {
        "nvidia" if g.seen_by.iter().any(|s| s == "nvml") => v.push("cuda"),
        "apple" => v.push("metal"),
        "amd" if f.runtimes.iter().any(|r| r == "rocm") => v.push("rocm"),
        "intel" if f.runtimes.iter().any(|r| r == "openvino") => v.push("openvino"),
        _ => {}
    }
    if has_api(g, "metal") && !v.contains(&"metal") {
        v.push("metal");
    }
    if has_api(g, "vulkan") {
        v.push("vulkan");
    }
    if has_api(g, "dx12") && f.os == "windows" {
        v.push("directml");
    }
    if v.is_empty() && has_api(g, "gl") {
        v.push("gles");
    }
    v
}

/// Usable VRAM for model weights: NVML free MiB, Apple unified = half of RAM,
/// unknown = 0 (integrated cards without a number are offered but not primary).
fn usable_vram(g: &Gpu, f: &Facts) -> u64 {
    if let Some(free) = g.vram_free_mib {
        return free;
    }
    if g.vendor == "apple" {
        return f.ram_total_mib / 2;
    }
    0
}

/// Build the device profile. `policy`: "auto" (use what is found) or "cpu".
pub fn profile(f: &Facts, policy: &str) -> Profile {
    let ram = f.ram_limit_mib.unwrap_or(f.ram_total_mib);
    let total = f.logical_threads.max(1);
    let bus = usize::from(total >= 4);
    let compute = total - bus;

    // Best real GPU: skip software rasterisers, rank by usable VRAM then discrete.
    let mut gpus: Vec<&Gpu> = f.gpus.iter().filter(|g| g.kind != "software" && !gpu_backends(g, f).is_empty()).collect();
    gpus.sort_by_key(|g| std::cmp::Reverse((usable_vram(g, f), g.kind == "discrete")));

    let mut backends: Vec<String> = Vec::new();
    for g in &gpus {
        for b in gpu_backends(g, f) {
            if !backends.iter().any(|x| x == b) {
                backends.push(b.into());
            }
        }
    }
    if !f.npus.is_empty() && f.runtimes.iter().any(|r| r == "openvino") {
        backends.push("openvino-npu".into());
    }
    backends.push("cpu".into());

    let cpu_only = policy.eq_ignore_ascii_case("cpu");
    let best = gpus.first().copied();
    let vram = best.map_or(0, |g| usable_vram(g, f));
    // A GPU tier still needs host RAM to stage weights (2 GiB floor).
    let use_gpu = !cpu_only && vram >= 2048 && ram >= 2048;
    let primary = if use_gpu { backends[0].clone() } else { "cpu".to_string() };

    // Tier table (RAM decides CPU tiers; usable VRAM decides GPU tiers).
    let tier = match (use_gpu, vram, ram, total) {
        (true, v, _, _) if v >= 12_288 => "gpu-large",
        (true, v, _, _) if v >= 6_144 => "gpu-medium",
        (true, _, _, _) => "gpu-small",
        (_, _, r, _) if r < 1_024 => "micro",
        (_, _, r, _) if r < 4_096 => "small",
        (_, _, r, t) if r < 16_384 || t < 8 => "medium",
        _ => "large",
    };
    let role = |backend: &str, size: &str, threads: usize| Role { backend: backend.into(), size: size.into(), threads };
    let g = primary.as_str();
    let (asr, tts, emotion, reasoning) = match tier {
        "micro" => (role("cpu", "remote", 0), role("cpu", "tiny", 1), role("cpu", "remote", 0), role("cpu", "remote", 0)),
        "small" => (role("cpu", "small", 1), role("cpu", "small", 1), role("cpu", "tiny", 1), role("cpu", "remote", 0)),
        "medium" => (role("cpu", "small", 1), role("cpu", "medium", 1), role("cpu", "small", 1), role("cpu", "small", compute.saturating_sub(3).max(1))),
        "large" => (role("cpu", "large", 2), role("cpu", "large", 2), role("cpu", "medium", 1), role("cpu", "medium", compute.saturating_sub(5).max(1))),
        "gpu-small" => (role(g, "small", 1), role("cpu", "medium", 2), role(g, "small", 1), role(g, "small", 1)),
        "gpu-medium" => (role(g, "medium", 1), role(g, "large", 1), role(g, "medium", 1), role(g, "medium", 1)),
        _ => (role(g, "large", 1), role(g, "large", 1), role(g, "large", 1), role(g, "large", 1)),
    };

    let ort_providers = backends
        .iter()
        .filter_map(|b| match b.as_str() {
            "cuda" => Some("CUDAExecutionProvider"),
            "rocm" => Some("ROCMExecutionProvider"),
            "openvino" | "openvino-npu" => Some("OpenVINOExecutionProvider"),
            "metal" => Some("CoreMLExecutionProvider"),
            "directml" => Some("DmlExecutionProvider"),
            "cpu" => Some("CPUExecutionProvider"),
            _ => None,
        })
        .filter(|p| !cpu_only || *p == "CPUExecutionProvider")
        .map(String::from)
        .fold(Vec::new(), |mut v, p| {
            if !v.contains(&p) {
                v.push(p);
            }
            v
        });

    Profile {
        schema: SCHEMA,
        policy: if cpu_only { "cpu".into() } else { "auto".into() },
        tier: tier.into(),
        backends,
        primary,
        accel_gpu: if use_gpu { best.map(|g| g.name.clone()) } else { None },
        threads: Threads { total, bus, compute },
        asr,
        tts,
        emotion,
        reasoning,
        ort_providers,
        facts: f.clone(),
    }
}

/// `GALION_ACCEL` (auto | cpu), default auto.
pub fn policy_from_env() -> String {
    std::env::var("GALION_ACCEL").unwrap_or_else(|_| "auto".into())
}
