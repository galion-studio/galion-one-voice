// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `GALION_SAMANTHA_REF` is a local wav. Its sha256 must start with `10ac44dc`.

use crate::error::TtsError;
use crate::recipe::{sha_prefix_ok, REF_SHA_PREFIX};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub path: PathBuf,
    pub sha256: String,
}

impl Reference {
    /// Hash the file. Refuse anything whose digest does not start with the s711 prefix.
    pub fn from_file(path: &Path) -> Result<Self, TtsError> {
        if !path.is_file() {
            return Err(TtsError::new(
                "reference",
                format!("reference file missing at {}", path.display()),
            ));
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !name.ends_with(".wav") {
            return Err(TtsError::new("reference", "reference must be a wav path"));
        }
        let sha256 = sha256_file(path)?;
        if !sha_prefix_ok(&sha256) {
            return Err(TtsError::new(
                "reference",
                format!("sha256 {sha256} does not start with {REF_SHA_PREFIX}"),
            ));
        }
        Ok(Self {
            path: path.to_path_buf(),
            sha256,
        })
    }

    /// Does not read the file. The worker hashes again when it speaks.
    /// Callers that have a real clip use [`Self::from_file`].
    pub fn trusted_for_tests(path: PathBuf, sha256: impl Into<String>) -> Result<Self, TtsError> {
        let sha256 = sha256.into();
        if !sha_prefix_ok(&sha256) {
            return Err(TtsError::new(
                "reference",
                format!("sha256 {sha256} does not start with {REF_SHA_PREFIX}"),
            ));
        }
        Ok(Self { path, sha256 })
    }

    pub fn from_env() -> Result<Self, TtsError> {
        let raw = std::env::var("GALION_SAMANTHA_REF").map_err(|_| {
            TtsError::new(
                "reference",
                "GALION_SAMANTHA_REF is unset; the reference wav is not in the repo",
            )
        })?;
        let path = raw.trim();
        if path.is_empty() {
            return Err(TtsError::new("reference", "GALION_SAMANTHA_REF is empty"));
        }
        Self::from_file(Path::new(path))
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String, TtsError> {
    let mut file = File::open(path)
        .map_err(|err| TtsError::new("reference", format!("open {}: {err}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| TtsError::new("reference", format!("read {}: {err}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::REF_SHA_PREFIX;

    #[test]
    fn sha256_of_abc_matches_the_known_digest() {
        let digest = sha256_hex(b"abc");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_fixture_file_is_refused_when_its_prefix_is_not_s711() {
        let dir = std::env::temp_dir().join(format!("galion-s711-ref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.wav");
        std::fs::write(&path, b"not a samantha recording, only a prefix gate").unwrap();
        let err = Reference::from_file(&path).unwrap_err();
        assert_eq!(err.kind, "reference");
        assert!(err.detail.contains(REF_SHA_PREFIX), "{}", err.detail);
        let digest = sha256_file(&path).unwrap();
        assert!(!digest.starts_with(REF_SHA_PREFIX));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
