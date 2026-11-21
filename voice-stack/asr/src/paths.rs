// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::{Path, PathBuf};

use crate::error::AsrError;

pub const MODEL_REL: &str = "weights/asr/vosk-model-small-en-us-0.15";

pub fn lib_file_name() -> &'static str {
    if cfg!(windows) {
        "libvosk.dll"
    } else if cfg!(target_os = "macos") {
        "libvosk.dylib"
    } else {
        "libvosk.so"
    }
}

pub fn roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    push_unique(&mut out, PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo) = manifest.parent().and_then(|p| p.parent()) {
        push_unique(&mut out, repo.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur = cwd;
        for _ in 0..4 {
            push_unique(&mut out, cur.clone());
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                break;
            }
        }
    }
    out
}

fn push_unique(out: &mut Vec<PathBuf>, path: PathBuf) {
    if !out.iter().any(|p| p == &path) {
        out.push(path);
    }
}

pub fn model_dir_ok(path: &Path) -> bool {
    path.is_dir() && path.join("am/final.mdl").is_file() && path.join("conf/model.conf").is_file()
}

pub fn find_vosk_model() -> Result<PathBuf, AsrError> {
    if let Ok(explicit) = std::env::var("GALION_VOSK_MODEL") {
        let path = PathBuf::from(&explicit);
        if model_dir_ok(&path) {
            return Ok(path);
        }
        return Err(AsrError::new(format!(
            "GALION_VOSK_MODEL={explicit} is missing am/final.mdl or conf/model.conf. Run voice-stack/asr/scripts/fetch-models.sh. No transcript was invented."
        )));
    }
    let mut tried = Vec::new();
    for root in roots() {
        let path = root.join(MODEL_REL);
        tried.push(path.display().to_string());
        if model_dir_ok(&path) {
            return Ok(path);
        }
    }
    Err(AsrError::new(format!(
        "Vosk model not found (need am/final.mdl and conf/model.conf). Looked in {}. Run voice-stack/asr/scripts/fetch-models.sh. No transcript was invented.",
        tried.join(", ")
    )))
}

pub fn find_vosk_lib() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("GALION_VOSK_LIB") {
        let path = PathBuf::from(&explicit);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let name = lib_file_name();
    for root in roots() {
        let path = root.join("weights/asr/libvosk").join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn find_vosk_bin() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("GALION_VOSK_BIN") {
        let path = PathBuf::from(&explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut names = vec![PathBuf::from("galion-asr-vosk")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            names.push(dir.join("galion-asr-vosk"));
        }
    }
    for root in roots() {
        names.push(root.join("target/release/galion-asr-vosk"));
        names.push(root.join("target/debug/galion-asr-vosk"));
    }
    names.into_iter().find(|p| p.is_file())
}

pub fn cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("model name") {
                    if let Some((_, value)) = rest.split_once(':') {
                        return value.trim().to_string();
                    }
                }
            }
        }
    }
    "unknown".into()
}

pub fn env_u32(key: &str, default: u32) -> Result<u32, AsrError> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(value) if value.is_empty() => Ok(default),
        Ok(value) => value.parse::<u32>().map_err(|_| {
            AsrError::new(format!(
                "{key}={value} is not an integer. No transcript was invented."
            ))
        }),
    }
}
