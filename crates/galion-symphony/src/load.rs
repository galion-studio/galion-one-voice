// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Open a symphony pack through `galion-tensor`'s `.gq` reader and the clip ABI.

use crate::clip::{self, Clip};
use crate::manifest::{self, Manifest};
use crate::sha;
use crate::{Error, Result};
use galion_tensor::gq::{GqDType, GqFile, Prefault};
use std::path::{Path, PathBuf};

pub struct Loaded {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub tensors: Vec<GqFile>,
    pub clip: Option<Clip>,
}

pub fn open(dir: &Path, prefault: Prefault) -> Result<Loaded> {
    let sums_path = dir.join("SHA256SUMS");
    let sums = std::fs::read_to_string(&sums_path)
        .map_err(|e| Error::io(&sums_path.display().to_string(), e))?;
    let lines =
        galion_tensor::gq::parse_sha256sums(&sums).map_err(|e| Error::Bad(e.to_string()))?;
    if lines.is_empty() {
        return Err(Error::Bad("SHA256SUMS is empty".into()));
    }
    for line in &lines {
        let path = dir.join(&line.path);
        let got = sha::hash_file(&path).map_err(|e| Error::io(&path.display().to_string(), e))?;
        if got != line.sha256 {
            return Err(Error::Bad(format!("{} failed SHA256SUMS", line.path)));
        }
    }
    let manifest = manifest::read(dir)?;
    let mut tensors = Vec::new();
    for f in &manifest.files {
        let path = dir.join(&f.rel);
        let got = sha::hex(
            &sha::hash_file(&path).map_err(|e| Error::io(&path.display().to_string(), e))?,
        );
        if got != f.sha256 {
            return Err(Error::Bad(format!(
                "{} sha256 does not match the manifest",
                f.rel
            )));
        }
        let gq =
            GqFile::open(&path, prefault).map_err(|e| Error::Bad(format!("{}: {e}", f.rel)))?;
        gq.verify_all()
            .map_err(|e| Error::Bad(format!("{}: {e}", f.rel)))?;
        if gq.len() != f.tensors as usize {
            return Err(Error::Bad(format!("{} tensor count", f.rel)));
        }
        tensors.push(gq);
    }
    for rec in &manifest.tensors {
        let slot = manifest
            .files
            .iter()
            .position(|f| f.rel == rec.file)
            .ok_or_else(|| Error::Bad(rec.file.clone()))?;
        let gq = &tensors[slot];
        let entry = gq
            .entry(&rec.name)
            .ok_or_else(|| Error::Bad(format!("missing tensor {}", rec.name)))?;
        let dtype = match entry.dtype {
            GqDType::F32 => "F32",
            GqDType::BF16 => "BF16",
            GqDType::I8 => "I8",
        };
        if dtype != rec.dtype {
            return Err(Error::Bad(format!("{} dtype", rec.name)));
        }
        let dims: Vec<u64> = entry.dims.iter().map(|&d| d as u64).collect();
        if dims != rec.shape {
            return Err(Error::Bad(format!("{} shape", rec.name)));
        }
    }
    let clip = match &manifest.clip {
        None => None,
        Some(rec) => {
            let path = dir.join(&rec.rel);
            let file_sha = sha::hex(
                &sha::hash_file(&path).map_err(|e| Error::io(&path.display().to_string(), e))?,
            );
            if file_sha != rec.file_sha256 {
                return Err(Error::Bad("clip file sha256".into()));
            }
            let clip = clip::open(&path)?;
            if sha::hex(&clip.header.payload_sha256) != rec.payload_sha256 {
                return Err(Error::Bad("clip payload sha256".into()));
            }
            if clip.header.sample_rate != rec.sample_rate
                || clip.header.channels != rec.channels
                || clip.header.bits != rec.bits
                || clip.header.duration_ms != rec.duration_ms
                || clip.header.texture != rec.texture
                || clip.header.texture_strip != 0
                || clip.header.persona != manifest.persona
            {
                return Err(Error::Bad("clip header does not match the manifest".into()));
            }
            Some(clip)
        }
    };
    Ok(Loaded {
        manifest,
        dir: dir.to_path_buf(),
        tensors,
        clip,
    })
}
