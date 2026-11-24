// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 8 gate.
//!
//! Command (prints the three decision lines, no timing, no network):
//!
//! ```text
//! cargo test -p horbm-boot fixture_without_a_gpu_and_fixture_with_a_gpu_both_select_the_cpu_brain -- --nocapture
//! ```
//!
//! A `hardware.json` with no GPU and a `hardware.json` with a GPU both select
//! brain `horbm` and mouth `cpu`. `llm` `api` and `llm` `local-gpu` stay on
//! the report. They are not the brain.

use horbm_boot::{BootError, VoiceBoot, BRAIN_NAME, MOUTH_DEVICE};
use std::path::{Path, PathBuf};
use std::process::Command;

const NO_GPU: &str = include_str!("../fixtures/hardware-no-gpu.json");
const GPU: &str = include_str!("../fixtures/hardware-gpu.json");
const GPU_LOW: &str = include_str!("../fixtures/hardware-gpu-low.json");

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// Absolute path Cargo gives this integration test. The name keeps the hyphen.
/// On Windows the value already ends in `.exe`.
fn boot_bin() -> &'static str {
    env!("CARGO_BIN_EXE_horbm-boot")
}

fn run_bin(path: &Path) -> std::process::Output {
    let bin = boot_bin();
    Command::new(bin)
        .arg(path)
        .output()
        .unwrap_or_else(|err| panic!("spawn {bin}: {err}"))
}

fn boot_table() -> std::collections::HashMap<String, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut in_boot = false;
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_boot = trimmed == "[boot]";
            continue;
        }
        if !in_boot || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        map.insert(
            key.trim().to_string(),
            value.trim().trim_matches('"').to_string(),
        );
    }
    map
}

fn assert_cpu(boot: &VoiceBoot) {
    assert_eq!(boot.brain, BRAIN_NAME, "{boot}");
    assert_eq!(boot.brain, "horbm", "{boot}");
    assert_eq!(boot.mouth_device, MOUTH_DEVICE, "{boot}");
    assert_eq!(boot.mouth_device, "cpu", "{boot}");
    assert!(boot.selects_cpu_brain(), "{boot}");
    assert_ne!(boot.brain, boot.probe_llm, "{boot}");
}

#[test]
fn the_lock_names_the_same_cpu_brain() {
    let table = boot_table();
    assert_eq!(table.get("brain").map(String::as_str), Some(BRAIN_NAME));
    assert_eq!(
        table.get("mouth_device").map(String::as_str),
        Some(MOUTH_DEVICE)
    );
    assert_eq!(
        table.get("gpu_moves_mouth").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        table.get("probe_llm_selects_brain").map(String::as_str),
        Some("false")
    );
    assert_eq!(table.get("report_gpu").map(String::as_str), Some("true"));
}

#[test]
fn fixture_without_a_gpu_and_fixture_with_a_gpu_both_select_the_cpu_brain() {
    let cases = [
        (
            "hardware-no-gpu.json",
            NO_GPU,
            false,
            "",
            "api",
            "cpu-only",
            false,
        ),
        (
            "hardware-gpu.json",
            GPU,
            true,
            "RTX 4090",
            "local-gpu",
            "gpu-room",
            true,
        ),
        (
            "hardware-gpu-low.json",
            GPU_LOW,
            true,
            "RTX 4090",
            "api",
            "gpu-low",
            false,
        ),
    ];
    let mut lines = Vec::new();
    for (file, body, gpu, name, llm, rule, weights) in cases {
        let boot = VoiceBoot::select(body.as_bytes()).expect(file);
        assert_cpu(&boot);
        assert_eq!(boot.gpu_reported(), gpu, "{boot}");
        assert_eq!(boot.gpu.present, gpu, "{boot}");
        assert_eq!(boot.gpu.name, name, "{boot}");
        assert_eq!(boot.probe_llm, llm, "{boot}");
        assert_eq!(boot.probe_rule, rule, "{boot}");
        assert!(boot.probe_mouth, "{boot}");
        assert_eq!(boot.probe_weights, weights, "{boot}");
        if gpu && rule == "gpu-room" {
            assert_eq!(boot.gpu.free_mib, 23564, "{boot}");
            assert_eq!(boot.gpu.total_mib, 24564, "{boot}");
            assert_eq!(boot.gpu.used_mib, 1000, "{boot}");
        }
        if rule == "gpu-low" {
            assert_eq!(boot.gpu.free_mib, 4000, "{boot}");
        }
        let out = run_bin(&fixture_path(file));
        assert!(out.status.success(), "{file} status {:?}", out.status);
        let stdout = String::from_utf8(out.stdout).expect("utf-8");
        assert!(
            out.stderr.is_empty(),
            "{file} stderr {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(stdout, format!("{boot}\n"), "{file}");
        lines.push(stdout);
    }
    println!("phase 8 gate");
    for line in &lines {
        print!("{line}");
    }
}

#[test]
fn every_probe_llm_line_keeps_the_named_cpu_brain() {
    let gpus = [
        "null",
        r#"{"name":"RTX 4090","total_mib":24564,"used_mib":1000,"free_mib":23564}"#,
        r#"{"name":"drm","total_mib":0,"used_mib":0,"free_mib":0}"#,
    ];
    for llm in ["api", "local-gpu", "lan", "cpu-light", "later-token"] {
        for gpu in gpus {
            let body = format!(
                r#"{{"llm":"{llm}","rule":"ignored","gpu":{gpu},"weights":true,"mouth":false,"brain":"api","mouth_device":"cuda"}}"#
            );
            let boot = VoiceBoot::select(body.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
            assert_cpu(&boot);
            assert_eq!(boot.probe_llm, llm);
            assert!(boot.probe_weights);
            assert!(!boot.probe_mouth);
            assert_eq!(boot.gpu.present, gpu != "null");
        }
    }
}

#[test]
fn a_drm_card_is_reported_and_the_mouth_stays_on_cpu() {
    let body = r#"{
        "gpu": {"name":"drm","total_mib":0,"used_mib":0,"free_mib":0},
        "llm": "lan",
        "rule": "wan-down-lan-up",
        "weights": false,
        "mouth": true
    }"#;
    let boot = VoiceBoot::select(body.as_bytes()).unwrap();
    assert_cpu(&boot);
    assert!(boot.gpu_reported());
    assert_eq!(boot.gpu.name, "drm");
    assert_eq!(boot.gpu.free_mib, 0);
    assert_eq!(boot.probe_llm, "lan");
}

#[test]
fn a_small_cpu_box_still_selects_the_named_brain() {
    let body = r#"{"llm":"api","rule":"cpu-only","mouth":false,"weights":false,"gpu":null,"ram_total_mib":2048}"#;
    let boot = VoiceBoot::select(body.as_bytes()).unwrap();
    assert_cpu(&boot);
    assert!(!boot.gpu_reported());
    assert!(!boot.probe_mouth);
}

#[test]
fn free_mib_is_reported_as_written() {
    let body =
        r#"{"llm":"local-gpu","gpu":{"name":"card","total_mib":10,"used_mib":2,"free_mib":1}}"#;
    let boot = VoiceBoot::select(body.as_bytes()).unwrap();
    assert_cpu(&boot);
    assert_eq!(boot.gpu.free_mib, 1);
    assert_eq!(boot.gpu.total_mib, 10);
    assert_eq!(boot.gpu.used_mib, 2);
}

#[test]
fn unknown_keys_and_key_order_do_not_change_the_selection() {
    let shuffled = r#"{"at":"2026-01-01T00:00:00+01:00","line":"x","rule":"cpu-only","llm":"api","extra":1,"mouth":true,"weights":false,"gpu":null}"#;
    let boot = VoiceBoot::select(shuffled.as_bytes()).unwrap();
    assert_cpu(&boot);
    assert!(!boot.gpu_reported());
    let stamped = NO_GPU.replace("2026-11-18T12:00:00+01:00", "2026-11-24T00:00:00+01:00");
    let a = VoiceBoot::select(NO_GPU.as_bytes()).unwrap();
    let b = VoiceBoot::select(stamped.as_bytes()).unwrap();
    assert_eq!(a.brain, b.brain);
    assert_eq!(a.mouth_device, b.mouth_device);
    assert_eq!(a.gpu, b.gpu);
    assert_eq!(a.probe_llm, b.probe_llm);
}

#[test]
fn a_utf8_bom_still_selects_the_cpu_brain() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(NO_GPU.as_bytes());
    let boot = VoiceBoot::select(&bytes).unwrap();
    assert_cpu(&boot);
    assert!(!boot.gpu_reported());
}

#[test]
fn machine_json_is_refused() {
    let body = br#"{"provider":"gpu","gpu":"RTX 4090","weights":true,"mouth":true}"#;
    let err = VoiceBoot::select(body).unwrap_err();
    assert_eq!(err, BootError::MachineDocument);
    let dir = std::env::temp_dir().join(format!("horbm-boot-machine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("machine.json");
    std::fs::write(&path, body).unwrap();
    let out = run_bin(&path);
    assert_eq!(out.status.code(), Some(2));
    let err_text = String::from_utf8(out.stderr).unwrap();
    assert!(err_text.contains("hardware.json"), "{err_text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_document_does_not_select_a_gpu_mouth() {
    let cases: [&[u8]; 5] = [
        b"",
        b"[]",
        b"null",
        br#"{"gpu":[]}"#,
        br#"{"llm":1,"gpu":null}"#,
    ];
    for body in cases {
        assert!(
            VoiceBoot::select(body).is_err(),
            "{}",
            String::from_utf8_lossy(body)
        );
    }
    let missing = VoiceBoot::select(br#"{"gpu":{"name":"RTX 4090","total_mib":1}}"#).unwrap_err();
    assert!(matches!(missing, BootError::GpuShape(_)));
}

#[test]
fn an_empty_object_still_names_the_cpu_brain() {
    let boot = VoiceBoot::select(b"{}").unwrap();
    assert_cpu(&boot);
    assert!(!boot.gpu_reported());
    assert!(boot.probe_llm.is_empty());
}

#[test]
fn the_binary_reads_the_fixture_path() {
    let boot = VoiceBoot::open(fixture_path("hardware-no-gpu.json")).unwrap();
    assert_cpu(&boot);
    assert_eq!(boot.probe_llm, "api");
    let missing = VoiceBoot::open(fixture_path("missing.json")).unwrap_err();
    assert!(matches!(missing, BootError::Io(_)));
}
