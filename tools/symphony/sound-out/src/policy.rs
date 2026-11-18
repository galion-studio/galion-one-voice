// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Which backend speaks. Piper is the realtime CPU default. Chatterbox is an
//! optional offline path. Moshi is GPU-only and only runs when named.
//! A missing voice pack is the Piper stand-in. Nothing is downloaded.

use std::path::Path;

use crate::backend::ClonerBackend;
use crate::chatterbox::ChatterboxBackend;
use crate::device::detect;
use crate::moshi::MoshiStub;
use crate::piper::PiperBackend;

#[derive(Clone, Debug)]
pub struct Policy {
    pub default_backend: String,
    pub piper_runtime: String,
    pub piper_model: String,
    pub piper_config: String,
}

impl Policy {
    pub fn builtin() -> Self {
        Self {
            default_backend: "piper".into(),
            piper_runtime: String::new(),
            piper_model: String::new(),
            piper_config: String::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut policy = Self::builtin();
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"').to_string();
            match (section.as_str(), key) {
                ("", "default_backend") => policy.default_backend = value,
                ("piper", "runtime") => policy.piper_runtime = value,
                ("piper", "model") => policy.piper_model = value,
                ("piper", "config") => policy.piper_config = value,
                _ => {}
            }
        }
        if policy.default_backend != "piper" {
            return Err(format!(
                "sound-out default_backend is {}, want piper",
                policy.default_backend
            ));
        }
        Ok(policy)
    }

    /// `name` empty uses the config default. Chatterbox and Moshi have to be named.
    pub fn open(
        &self,
        name: Option<&str>,
        device: Option<&str>,
    ) -> Result<Box<dyn ClonerBackend>, String> {
        let name = name.unwrap_or(self.default_backend.as_str());
        match name {
            "piper" | "piper-stand-in" => Ok(Box::new(PiperBackend::resolve_files(
                nonempty(&self.piper_runtime),
                nonempty(&self.piper_model),
                nonempty(&self.piper_config),
            ))),
            "chatterbox" => Ok(Box::new(ChatterboxBackend::present())),
            "moshi" => {
                let _ = device;
                let seen = detect();
                if !seen.gpu_present {
                    return Err(
                        "moshi is GPU-only and no GPU is visible. It is not the default.".into(),
                    );
                }
                Ok(Box::new(MoshiStub))
            }
            other => Err(format!(
                "unknown backend {other}. piper is the default; chatterbox and moshi are opt-in"
            )),
        }
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
