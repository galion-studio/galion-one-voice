// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Compile a probe only when the open pull request's source is on disk.
//! The probe runs in its own target directory so it does not take this
//! package's cargo lock.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Sense;

pub struct Probe {
    pub lines: Vec<(String, String)>,
}

pub fn ear_lib() -> PathBuf {
    manifest("../../../crates/horbm-ear/src/lib.rs")
}

pub fn ear_eot() -> PathBuf {
    manifest("../../../crates/horbm-ear/src/eot.rs")
}

pub fn ear_listen() -> PathBuf {
    manifest("../../../crates/horbm-ear/src/listen.rs")
}

pub fn piper_mouth() -> PathBuf {
    manifest("../horbm-duplex/src/piper_mouth.rs")
}

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

pub fn run_named(name: &str, sense: Sense, cargo_toml: &str) -> Result<Probe, String> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("probes")
        .join(format!("{name}.rs"));
    if !source.is_file() {
        return Err(format!("probe source {} is missing", source.display()));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "horbm-e2e-probe-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|err| format!("probe dir: {err}"))?;
    let outcome = run_in(&dir, &source, sense, cargo_toml);
    let _ = fs::remove_dir_all(&dir);
    outcome
}

fn run_in(dir: &Path, source: &Path, sense: Sense, cargo_toml: &str) -> Result<Probe, String> {
    fs::write(dir.join("Cargo.toml"), cargo_toml)
        .map_err(|err| format!("probe manifest: {err}"))?;
    fs::create_dir_all(dir.join("src")).map_err(|err| format!("probe src: {err}"))?;
    fs::copy(source, dir.join("src/main.rs")).map_err(|err| format!("probe copy: {err}"))?;
    let target = dir.join("target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", &target);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    match sense {
        Sense::Off => {
            command.env("HORBM_E2E_SENSE", "0");
        }
        Sense::On => {
            command.env_remove("HORBM_E2E_SENSE");
        }
    }
    let output = command
        .output()
        .map_err(|err| format!("probe cargo: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!("probe failed: {tail}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = Vec::new();
    for line in stdout.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        if key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !value.is_empty()
            && !value.contains(' ')
        {
            lines.push((key.to_string(), value.to_string()));
        }
    }
    if lines.is_empty() {
        return Err("probe printed no fields".into());
    }
    Ok(Probe { lines })
}
