// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Whole-file pins for the clone interchange. Same three columns as
//! `scripts/clone/PINS.sha256`. The bytes are not in git.

use crate::{Error, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    pub rel: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

pub const PINS: &[Pin] = &[
    Pin {
        rel: "shared/s3gen.safetensors",
        sha256: "2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e",
        size: 1_056_484_620,
    },
    Pin {
        rel: "shared/s3gen_meanflow.safetensors",
        sha256: "d65cb687a2ed581ee6cc297e919ffefa63386944f42364ae13b78a594945514f",
        size: 1_064_875_036,
    },
    Pin {
        rel: "shared/ve.safetensors",
        sha256: "f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c",
        size: 5_695_784,
    },
    Pin {
        rel: "nano/t3_nano_v1.safetensors",
        sha256: "72b110185087d945dbdf54dee4e333848e1811bdd5fd6cb16ceb8da50006f0c9",
        size: 869_899_204,
    },
    Pin {
        rel: "turbo/t3_turbo_v1.safetensors",
        sha256: "fcf1f8c1d651bb7e3acd69ee5be269b4ac10c02980b7708213d598bc9f7cdf87",
        size: 1_915_480_052,
    },
];

pub fn pin_for(rel: &str) -> Option<&'static Pin> {
    PINS.iter().find(|p| p.rel == rel)
}

/// `Ok(true)` when `scripts/clone/PINS.sha256` and [`PINS`] name the same files.
pub fn matches_script() -> bool {
    let text = include_str!("../../../scripts/clone/PINS.sha256");
    let mut found = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(rel) = parts.next() else { continue };
        let Some(sha) = parts.next() else { continue };
        let Some(size) = parts.next().and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        found.push((rel, sha, size));
    }
    if found.len() != PINS.len() {
        return false;
    }
    PINS.iter().all(|p| {
        found
            .iter()
            .any(|(rel, sha, size)| *rel == p.rel && *sha == p.sha256 && *size == p.size)
    })
}

pub fn verify_file(path: &Path, pin: &Pin) -> Result<()> {
    let meta = std::fs::metadata(path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    if meta.len() != pin.size {
        return Err(Error::Refused(format!(
            "{} is {} bytes; pin {} is {} bytes",
            path.display(),
            meta.len(),
            pin.rel,
            pin.size
        )));
    }
    let got = crate::sha::hex(
        &crate::sha::hash_file(path).map_err(|e| Error::io(&path.display().to_string(), e))?,
    );
    if got != pin.sha256 {
        return Err(Error::Refused(format!(
            "{} sha256 does not match pin {}",
            path.display(),
            pin.rel
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_and_table_agree() {
        assert!(matches_script());
    }
}
