# galion-hw bricks

Each brick is an existing, working open component, pinned in `Cargo.toml`.
`galion-hw` only joins them (thin glue in `src/lib.rs`).

| brick | version | licence | what it gives | upstream |
|---|---|---|---|---|
| sysinfo | =0.36.1 | MIT | OS name/version, kernel, CPU brand, physical cores, logical threads, RAM total/available, cgroup RAM cap | https://github.com/GuillaumeGomez/sysinfo |
| nvml-wrapper | =0.11.0 (+ nvml-wrapper-sys 0.9.1) | MIT OR Apache-2.0 | NVIDIA name, VRAM total/free, driver, CUDA driver, compute capability; libnvidia-ml is dlopened at run time | https://github.com/rust-nvml/nvml-wrapper |
| wgpu | =23.0.1 (same major as apps/nebula-voice-v2) | MIT OR Apache-2.0 | every GPU adapter over Vulkan / Metal / DX12 / GL with vendor id and device type (AMD, Intel, Apple, Mali, Adreno, VideoCore, software) | https://github.com/gfx-rs/wgpu |
| serde + serde_json | 1.x | MIT OR Apache-2.0 | JSON profile / facts in and out | https://github.com/serde-rs |
| galion-tensor `features::detected` | in tree | MIT (Galion) | SIMD level: avx512 / avx2 / neon / scalar | crates/galion-tensor/src/features.rs |
| obsidian-open-mind `galion-boot/src/hardware.rs` | pattern only | MIT (Galion) | the "check /sys and /dev, never fail" style used for NPU nodes and amdgpu sysfs VRAM | other repo, not a dependency |

Not used, on purpose: `ort` and `candle`. LOCKS.toml `formats.forbidden_runtimes`
bans both from the main workspace (ort is allowed only in the separate
`runtimes/galion-piper-rt` workspace). The profile therefore only *names* ORT
execution providers (`ort_providers`) for that out-of-process runtime to read.

Licence texts for the crates ship with their sources on crates.io
(`~/.cargo/registry/src/*/<crate>-<version>/LICENSE*`).

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
