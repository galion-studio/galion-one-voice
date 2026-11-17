// G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
// Developed and distributed by Galion Labs
// Perpetual Open MIT License
// Copyright © 2026 Galion Labs. All rights reserved.
use galion_hw::{detect, merge_adapter, nodes, profile, Facts, Gpu};

fn sim(name: &str) -> Facts {
    let p = format!("{}/sim/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn rtx4060() -> Facts {
    Facts {
        os: "windows".into(),
        arch: "x86_64".into(),
        logical_threads: 16,
        physical_cores: 10,
        ram_total_mib: 65_200,
        gpus: vec![Gpu {
            name: "NVIDIA GeForce RTX 4060 Laptop GPU".into(),
            vendor: "nvidia".into(),
            vendor_id: 0x10de,
            kind: "discrete".into(),
            seen_by: vec!["nvml".into()],
            vram_total_mib: Some(8188),
            vram_free_mib: Some(6500),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn real_detect_never_panics_and_has_cpu() {
    let f = detect();
    assert!(f.logical_threads >= 1);
    assert!(f.ram_total_mib > 0);
    let p = profile(&f, "auto");
    assert_eq!(p.backends.last().map(String::as_str), Some("cpu"));
    println!("{}", serde_json::to_string(&p).unwrap());
}

#[test]
fn geforce_takes_over_voice() {
    let mut f = rtx4060();
    merge_adapter(&mut f, "NVIDIA GeForce RTX 4060 Laptop GPU", 0x10de, "discrete", "vulkan", "");
    merge_adapter(&mut f, "NVIDIA GeForce RTX 4060 Laptop GPU", 0x10de, "discrete", "dx12", "");
    assert_eq!(f.gpus.len(), 1, "one card, three views");
    let p = profile(&f, "auto");
    assert_eq!(p.primary, "cuda");
    assert_eq!(p.tier, "gpu-medium");
    assert_eq!(p.backends, ["cuda", "vulkan", "directml", "cpu"]);
    assert_eq!(p.asr.backend, "cuda");
    assert_eq!(p.ort_providers[0], "CUDAExecutionProvider");
    assert_eq!((p.threads.bus, p.threads.compute), (1, 15));
}

#[test]
fn cpu_policy_wins_over_gpu() {
    let p = profile(&rtx4060(), "cpu");
    assert_eq!(p.primary, "cpu");
    assert_eq!(p.tier, "large");
    assert!(p.accel_gpu.is_none());
    assert_eq!(p.ort_providers, ["CPUExecutionProvider"]);
    assert!([&p.asr, &p.tts, &p.emotion, &p.reasoning].iter().all(|r| r.backend == "cpu"));
}

#[test]
fn low_ram_no_gpu_is_small_cpu() {
    let p = profile(&sim("lowram-nogpu"), "auto");
    assert_eq!((p.tier.as_str(), p.primary.as_str()), ("small", "cpu"));
    assert_eq!(p.backends, ["cpu"]);
    assert_eq!(p.reasoning.size, "remote");
}

#[test]
fn router_is_micro_satellite() {
    let p = profile(&sim("router-micro"), "auto");
    assert_eq!(p.tier, "micro");
    assert_eq!(p.asr.size, "remote");
    assert_eq!(p.tts.size, "tiny");
}

#[test]
fn apple_uses_metal_unified_memory() {
    let p = profile(&sim("apple-m2"), "auto");
    assert_eq!(p.primary, "metal");
    assert_eq!(p.tier, "gpu-medium");
    assert!(p.ort_providers.contains(&"CoreMLExecutionProvider".to_string()));
}

#[test]
fn amd_with_rocm() {
    let p = profile(&sim("amd-rocm"), "auto");
    assert_eq!(p.backends, ["rocm", "vulkan", "cpu"]);
    assert_eq!(p.tier, "gpu-large");
}

#[test]
fn phone_gpu_without_vram_number_stays_cpu_but_offers_vulkan() {
    let p = profile(&sim("phone-adreno"), "auto");
    assert_eq!(p.primary, "cpu");
    assert_eq!(p.backends, ["vulkan", "cpu"]);
    assert_eq!(p.tier, "medium");
}

#[test]
fn software_rasteriser_is_ignored() {
    let mut f = Facts { logical_threads: 4, ram_total_mib: 8192, ..Default::default() };
    merge_adapter(&mut f, "llvmpipe (LLVM 15.0.7, 256 bits)", 0x10005, "software", "vulkan", "");
    assert_eq!(profile(&f, "auto").backends, ["cpu"]);
}

#[test]
fn fake_root_nodes_and_sysfs_vram() {
    let root = std::env::temp_dir().join(format!("galion-hw-root-{}", std::process::id()));
    let card = root.join("sys/class/drm/card0/device");
    std::fs::create_dir_all(&card).unwrap();
    std::fs::create_dir_all(root.join("dev/accel/accel0")).unwrap();
    std::fs::create_dir_all(root.join("opt/rocm")).unwrap();
    std::fs::write(card.join("vendor"), "0x1002\n").unwrap();
    std::fs::write(card.join("mem_info_vram_total"), (16u64 << 30).to_string()).unwrap();
    std::fs::write(card.join("mem_info_vram_used"), (1u64 << 30).to_string()).unwrap();
    let mut f = Facts { os: "linux".into(), logical_threads: 16, ram_total_mib: 32768, ..Default::default() };
    merge_adapter(&mut f, "AMD Radeon RX 7800 XT", 0x1002, "discrete", "vulkan", "");
    nodes(&mut f, &root);
    assert_eq!(f.npus, ["/dev/accel/accel0"]);
    assert_eq!(f.runtimes, ["rocm"]);
    assert_eq!((f.gpus[0].vram_total_mib, f.gpus[0].vram_free_mib), (Some(16384), Some(15360)));
    assert_eq!(profile(&f, "auto").primary, "rocm");
}

#[test]
fn nested_cgroup_cap_is_read() {
    let root = std::env::temp_dir().join(format!("galion-hw-cg-{}", std::process::id()));
    let scope = root.join("sys/fs/cgroup/user.slice/run-x.scope");
    std::fs::create_dir_all(&scope).unwrap();
    std::fs::create_dir_all(root.join("proc/self")).unwrap();
    std::fs::write(root.join("proc/self/cgroup"), "0::/user.slice/run-x.scope\n").unwrap();
    std::fs::write(scope.join("memory.max"), "943718400\n").unwrap();
    std::fs::write(root.join("sys/fs/cgroup/user.slice/memory.max"), "max\n").unwrap();
    assert_eq!(galion_hw::own_cgroup_cap_mib(&root), Some(900));
}

#[test]
fn gpu_needs_host_ram_too() {
    let mut f = rtx4060();
    f.ram_limit_mib = Some(900);
    let p = profile(&f, "auto");
    assert_eq!((p.tier.as_str(), p.primary.as_str()), ("micro", "cpu"));
    assert_eq!(p.backends[0], "cuda", "GPU is still listed");
}
