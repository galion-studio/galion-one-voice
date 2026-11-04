// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Read clone safetensors (interchange only) and write a symphony pack.

use crate::clip::{self, WrapOpts};
use crate::gq_write::{self, GqPart};
use crate::manifest::{ClipRec, FileRec, Manifest, TensorRec, FILE_NAME};
use crate::persona::{self, galion_name, ModelId, PersonaId};
use crate::pins::{self, pin_for};
use crate::safetensors::{self, refused_extension};
use crate::sha;
use crate::{Error, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct TranslateOpts {
    pub input: PathBuf,
    pub output: PathBuf,
    pub persona: PersonaId,
    pub model: ModelId,
    pub host: crate::host_cpu::HostId,
    pub wav: Option<PathBuf>,
    /// Fixture packs skip the published weight pins and the published clip hash.
    pub fixture: bool,
    pub force: bool,
}

const AUDIO_EXT: &[&str] = &["wav", "mp3", "flac", "mp4", "mkv", "mov"];

pub fn translate(opts: &TranslateOpts) -> Result<Manifest> {
    if opts.output.starts_with(&opts.input) {
        return Err(Error::Refused(
            "output directory must sit outside the interchange directory".into(),
        ));
    }
    if opts.output.join(FILE_NAME).is_file() && !opts.force {
        return Err(Error::Refused(format!(
            "{} already holds a pack (pass --force)",
            opts.output.display()
        )));
    }
    refuse_foreign_tree(&opts.input)?;
    let mut files = Vec::new();
    for rel in opts.model.files() {
        let path = opts.input.join(rel);
        if !path.is_file() {
            return Err(Error::Bad(format!("missing interchange file {rel}")));
        }
        if !opts.fixture {
            let pin = pin_for(rel).ok_or_else(|| Error::Bad(format!("no pin for {rel}")))?;
            pins::verify_file(&path, pin)?;
        }
        files.push(((*rel).to_string(), path));
    }
    std::fs::create_dir_all(opts.output.join("tensors"))
        .map_err(|e| Error::io(&opts.output.display().to_string(), e))?;
    let mut file_recs = Vec::new();
    let mut tensor_recs = Vec::new();
    let mut used_names = BTreeSet::new();
    for (rel, path) in &files {
        let file_name = Path::new(rel)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let sector = persona::sector(file_name)?;
        let st = safetensors::read_safetensors(path)?;
        let mut parts = Vec::new();
        for t in &st.tensors {
            let name = galion_name(sector, &t.name, &mut used_names)?;
            parts.push(GqPart {
                name: name.clone(),
                dtype: gq_write::dtype_of(t.dtype),
                dims: t.shape.clone(),
                src: path.clone(),
                src_off: t.file_off,
                len: t.len,
                sha256: t.sha256,
            });
            tensor_recs.push(TensorRec {
                name,
                file: format!("tensors/{sector}.gq"),
                dtype: match t.dtype {
                    safetensors::StDtype::F32 => "F32".into(),
                    safetensors::StDtype::BF16 => "BF16".into(),
                },
                shape: t.shape.clone(),
                source: t.name.clone(),
            });
        }
        parts.sort_by(|a, b| a.name.cmp(&b.name));
        let gq_rel = format!("tensors/{sector}.gq");
        let gq_path = opts.output.join(&gq_rel);
        gq_write::write_gq(&gq_path, &parts)?;
        let digest = sha::hex(
            &sha::hash_file(&gq_path).map_err(|e| Error::io(&gq_path.display().to_string(), e))?,
        );
        file_recs.push(FileRec {
            rel: gq_rel,
            sha256: digest,
            tensors: parts.len() as u32,
        });
    }
    tensor_recs.sort_by(|a, b| a.name.cmp(&b.name));
    let clip = match &opts.wav {
        None => None,
        Some(wav) => {
            let out = opts.output.join("clip.gclip");
            let header = clip::wrap(WrapOpts {
                persona: opts.persona,
                wav,
                out: &out,
                check_pin: !opts.fixture,
            })?;
            let file_sha = sha::hex(
                &sha::hash_file(&out).map_err(|e| Error::io(&out.display().to_string(), e))?,
            );
            Some(ClipRec {
                rel: "clip.gclip".into(),
                file_sha256: file_sha,
                payload_sha256: sha::hex(&header.payload_sha256),
                sample_rate: header.sample_rate,
                channels: header.channels,
                bits: header.bits,
                duration_ms: header.duration_ms,
                texture: header.texture,
            })
        }
    };
    let manifest = Manifest {
        persona: opts.persona,
        model: opts.model,
        host: opts.host.spec(),
        budget: crate::host_cpu::voice_budget(),
        texture: opts.persona.texture(),
        prosody: opts.persona.prosody(),
        emotions: opts
            .persona
            .emotions()
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        tags: opts
            .persona
            .tags()
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        tags_enabled: opts.persona.tags_enabled(),
        files: file_recs,
        tensors: tensor_recs,
        clip,
    };
    manifest.write(&opts.output)?;
    write_sums(&opts.output)?;
    Ok(manifest)
}

fn write_sums(dir: &Path) -> Result<()> {
    let mut rels = Vec::new();
    collect_rels(dir, dir, &mut rels)?;
    rels.retain(|r| r != "SHA256SUMS");
    rels.sort();
    let mut lines = String::new();
    for rel in rels {
        let path = dir.join(&rel);
        let digest = sha::hex(
            &sha::hash_file(&path).map_err(|e| Error::io(&path.display().to_string(), e))?,
        );
        lines.push_str(&digest);
        lines.push_str("  ");
        lines.push_str(&rel);
        lines.push('\n');
    }
    let path = dir.join("SHA256SUMS");
    std::fs::write(&path, lines).map_err(|e| Error::io(&path.display().to_string(), e))
}

fn collect_rels(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    let rd = std::fs::read_dir(dir).map_err(|e| Error::io(&dir.display().to_string(), e))?;
    for ent in rd {
        let ent = ent.map_err(|e| Error::io(&dir.display().to_string(), e))?;
        let path = ent.path();
        let ft = ent
            .file_type()
            .map_err(|e| Error::io(&path.display().to_string(), e))?;
        if ft.is_dir() {
            collect_rels(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
    Ok(())
}

fn refuse_foreign_tree(root: &Path) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0u32;
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir).map_err(|e| Error::io(&dir.display().to_string(), e))?;
        for ent in rd {
            seen += 1;
            if seen > 10_000 {
                return Err(Error::Bad("interchange tree is too large".into()));
            }
            let ent = ent.map_err(|e| Error::io(&dir.display().to_string(), e))?;
            let path = ent.path();
            let meta = std::fs::symlink_metadata(&path)
                .map_err(|e| Error::io(&path.display().to_string(), e))?;
            if meta.file_type().is_symlink() {
                return Err(Error::Refused(format!("symlink {}", path.display())));
            }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let lower = ext.to_ascii_lowercase();
            if refused_extension(&lower) || AUDIO_EXT.contains(&lower.as_str()) {
                return Err(Error::Refused(format!(
                    "{} is not symphony interchange (weights stay safetensors; audio is a .gclip outside this directory)",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}
