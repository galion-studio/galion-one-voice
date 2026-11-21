// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pre-rendered phrase cache. Raw s16le beside a json sidecar.
//! The directory must sit outside the git repo. A hit does not call the worker.

use crate::error::TtsError;
use crate::recipe::SentenceJob;
use crate::tts::Spoken;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct PhraseCache {
    dir: PathBuf,
}

impl PhraseCache {
    pub fn open(dir: &Path) -> Result<Self, TtsError> {
        if cache_dir_is_inside_repo(dir) {
            return Err(TtsError::new(
                "cache",
                format!(
                    "refusing to cache audio inside the repo at {}",
                    dir.display()
                ),
            ));
        }
        fs::create_dir_all(dir)
            .map_err(|err| TtsError::new("cache", format!("create {}: {err}", dir.display())))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn key(job: &SentenceJob) -> String {
        let raw = format!(
            "s711|{text}|{seed}|{sha}|{mode}|{exaggeration:.4}|{cfg:.4}|{device}|24000|1|16|raw",
            text = job.text,
            seed = job.seed,
            sha = job.reference_sha256,
            mode = crate::recipe::MODE,
            exaggeration = crate::recipe::EXAGGERATION,
            cfg = crate::recipe::CFG,
            device = job.device,
        );
        crate::reference::sha256_hex(raw.as_bytes())
    }

    pub fn get(&self, key: &str) -> Result<Option<Spoken>, TtsError> {
        let pcm_path = self.dir.join(format!("{key}.pcm"));
        let meta_path = self.dir.join(format!("{key}.json"));
        if !pcm_path.is_file() || !meta_path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(&pcm_path)
            .map_err(|err| TtsError::new("cache", format!("read {}: {err}", pcm_path.display())))?;
        let meta = fs::read_to_string(&meta_path).map_err(|err| {
            TtsError::new("cache", format!("read {}: {err}", meta_path.display()))
        })?;
        let value: serde_json::Value = serde_json::from_str(&meta)
            .map_err(|err| TtsError::new("cache", format!("meta {key}: {err}")))?;
        let rate_ok = value["sample_rate_hz"].as_u64() == Some(24_000);
        let channels_ok = value["channels"].as_u64() == Some(1);
        let bits_ok = value["bits"].as_u64() == Some(16);
        let encoding_ok = value["encoding"].as_str() == Some("s16le");
        let raw_ok = value["raw"].as_bool() == Some(true);
        if !rate_ok || !channels_ok || !bits_ok || !encoding_ok || !raw_ok {
            return Err(TtsError::new(
                "cache",
                format!("meta {key} is not raw 24 kHz 16-bit mono"),
            ));
        }
        let samples = decode_s16le(&bytes)?;
        let expected = value["samples"].as_u64().unwrap_or(0) as usize;
        if samples.len() != expected {
            return Err(TtsError::new(
                "cache",
                format!("meta {key} sample count does not match pcm"),
            ));
        }
        let device = value["device"].as_str().unwrap_or("cache").to_string();
        Ok(Some(Spoken {
            samples,
            device,
            worker_ttfa_us: None,
            from_cache: true,
        }))
    }

    pub fn put(&self, key: &str, job: &SentenceJob, spoken: &Spoken) -> Result<(), TtsError> {
        if spoken.samples.is_empty() {
            return Err(TtsError::new("cache", "refusing to cache empty pcm"));
        }
        let pcm_path = self.dir.join(format!("{key}.pcm"));
        let meta_path = self.dir.join(format!("{key}.json"));
        let bytes = encode_s16le(&spoken.samples);
        let meta = serde_json::json!({
            "sample_rate_hz": 24_000,
            "channels": 1,
            "bits": 16,
            "encoding": "s16le",
            "raw": true,
            "samples": spoken.samples.len(),
            "seed": job.seed,
            "recipe": "s711",
            "device": spoken.device,
            "ref_sha256_prefix": &job.reference_sha256[..8],
        });
        let tmp_pcm = self.dir.join(format!("{key}.pcm.tmp"));
        let tmp_meta = self.dir.join(format!("{key}.json.tmp"));
        fs::write(&tmp_pcm, bytes)
            .map_err(|err| TtsError::new("cache", format!("write {}: {err}", tmp_pcm.display())))?;
        fs::write(&tmp_meta, meta.to_string()).map_err(|err| {
            TtsError::new("cache", format!("write {}: {err}", tmp_meta.display()))
        })?;
        fs::rename(&tmp_pcm, &pcm_path).map_err(|err| {
            TtsError::new("cache", format!("rename {}: {err}", pcm_path.display()))
        })?;
        fs::rename(&tmp_meta, &meta_path).map_err(|err| {
            TtsError::new("cache", format!("rename {}: {err}", meta_path.display()))
        })?;
        Ok(())
    }
}

pub fn cache_dir_is_inside_repo(dir: &Path) -> bool {
    let repo = repo_root();
    let absolute = lexical_absolute(dir);
    absolute.starts_with(&repo)
}

pub fn repo_root() -> PathBuf {
    let joined = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    lexical_normalize(&joined)
}

fn lexical_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        lexical_normalize(path)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        lexical_normalize(&cwd.join(path))
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn decode_s16le(bytes: &[u8]) -> Result<Vec<i16>, TtsError> {
    if bytes.len() % 2 != 0 {
        return Err(TtsError::new("pcm", "pcm byte length is odd"));
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

pub fn encode_s16le(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_inside_the_crate_is_refused() {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(cache_dir_is_inside_repo(&crate_dir));
        assert!(PhraseCache::open(&crate_dir.join("cache")).is_err());
    }
}
