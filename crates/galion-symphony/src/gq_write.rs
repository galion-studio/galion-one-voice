// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Writer for Galion `.gq` v1. The reader lives in `galion-tensor`. This writer
//! exists so a clone interchange can land on that reader. Names must already
//! be in the `sym.` namespace.

use crate::{Error, Result};
use galion_tensor::gq::{
    GqDType, HEADER_LEN, MAGIC, MAX_NAME_LEN, MAX_RANK, MAX_TENSORS, MIN_ALIGN, VERSION,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GqPart {
    pub name: String,
    pub dtype: GqDType,
    pub dims: Vec<u64>,
    pub src: PathBuf,
    pub src_off: u64,
    pub len: u64,
    pub sha256: [u8; 32],
}

pub fn dtype_of(st: crate::safetensors::StDtype) -> GqDType {
    match st {
        crate::safetensors::StDtype::F32 => GqDType::F32,
        crate::safetensors::StDtype::BF16 => GqDType::BF16,
    }
}

/// Tensor payload already in memory. The pack test uses this. It is not a weight file.
#[derive(Debug, Clone)]
pub struct MemTensor {
    pub name: String,
    pub dtype: GqDType,
    pub dims: Vec<u64>,
    pub bytes: Vec<u8>,
}

enum Src<'a> {
    File { path: &'a Path, off: u64 },
    Mem(&'a [u8]),
}

struct Piece<'a> {
    name: &'a str,
    dtype: GqDType,
    dims: &'a [u64],
    sha256: &'a [u8; 32],
    len: u64,
    src: Src<'a>,
}

pub fn write_gq(path: &Path, parts: &[GqPart]) -> Result<()> {
    let pieces: Vec<Piece<'_>> = parts
        .iter()
        .map(|p| Piece {
            name: &p.name,
            dtype: p.dtype,
            dims: &p.dims,
            sha256: &p.sha256,
            len: p.len,
            src: Src::File {
                path: &p.src,
                off: p.src_off,
            },
        })
        .collect();
    write_pieces(path, &pieces)
}

pub fn write_gq_mem(path: &Path, parts: &[MemTensor]) -> Result<()> {
    let hashes: Vec<[u8; 32]> = parts.iter().map(|p| crate::sha::sha256(&p.bytes)).collect();
    let pieces: Vec<Piece<'_>> = parts
        .iter()
        .zip(hashes.iter())
        .map(|(p, hash)| Piece {
            name: &p.name,
            dtype: p.dtype,
            dims: &p.dims,
            sha256: hash,
            len: p.bytes.len() as u64,
            src: Src::Mem(&p.bytes),
        })
        .collect();
    write_pieces(path, &pieces)
}

fn write_pieces(path: &Path, parts: &[Piece<'_>]) -> Result<()> {
    if parts.is_empty() {
        return Err(Error::Bad(format!("{}: empty .gq", path.display())));
    }
    if parts.len() > MAX_TENSORS as usize {
        return Err(Error::Bad(format!("{}: too many tensors", path.display())));
    }
    let align = MIN_ALIGN as u64;
    let mut cursor = 0u64;
    let mut placed = Vec::with_capacity(parts.len());
    let mut seen = std::collections::BTreeSet::new();
    for p in parts {
        if !p.name.starts_with("sym.") || !p.name.bytes().all(|c| c.is_ascii_graphic() && c != b' ')
        {
            return Err(Error::Refused(format!(
                "tensor name {} is outside the sym. namespace",
                p.name
            )));
        }
        if p.name.len() > MAX_NAME_LEN || p.dims.len() > MAX_RANK {
            return Err(Error::Bad(format!("tensor {} exceeds .gq limits", p.name)));
        }
        if !seen.insert(p.name) {
            return Err(Error::Bad(format!("duplicate tensor {}", p.name)));
        }
        let es = match p.dtype {
            GqDType::F32 => 4u64,
            GqDType::BF16 => 2,
            GqDType::I8 => {
                return Err(Error::Refused(format!(
                    "{}: I8 is not emitted by the symphony translator",
                    p.name
                )));
            }
        };
        let numel = p
            .dims
            .iter()
            .try_fold(1u64, |a, &d| a.checked_mul(d))
            .ok_or_else(|| Error::Bad(format!("{}: shape", p.name)))?;
        if numel.saturating_mul(es) != p.len {
            return Err(Error::Bad(format!(
                "{}: length does not match dtype",
                p.name
            )));
        }
        cursor = cursor.div_ceil(align) * align;
        placed.push(cursor);
        cursor = cursor
            .checked_add(p.len)
            .ok_or_else(|| Error::Bad("data section overflow".into()))?;
    }
    let data_len = cursor;
    let mut table = Vec::new();
    for (p, off) in parts.iter().zip(&placed) {
        let nb = p.name.as_bytes();
        table.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        table.extend_from_slice(nb);
        table.push(p.dtype as u8);
        table.push(p.dims.len() as u8);
        table.extend_from_slice(&0u16.to_le_bytes());
        for d in p.dims {
            table.extend_from_slice(&d.to_le_bytes());
        }
        table.extend_from_slice(&0f32.to_le_bytes());
        table.extend_from_slice(&off.to_le_bytes());
        table.extend_from_slice(&p.len.to_le_bytes());
        table.extend_from_slice(p.sha256);
    }
    let table_len = table.len() as u64;
    let table_end = HEADER_LEN as u64 + table_len;
    let data_off = table_end.div_ceil(align) * align;
    let mut header = [0u8; HEADER_LEN];
    header[0..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&VERSION.to_le_bytes());
    header[12..16].copy_from_slice(&MIN_ALIGN.to_le_bytes());
    header[16..20].copy_from_slice(&(parts.len() as u32).to_le_bytes());
    header[24..32].copy_from_slice(&(HEADER_LEN as u64).to_le_bytes());
    header[32..40].copy_from_slice(&table_len.to_le_bytes());
    header[40..48].copy_from_slice(&data_off.to_le_bytes());
    header[48..56].copy_from_slice(&data_len.to_le_bytes());

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(&parent.display().to_string(), e))?;
    }
    let tmp = path.with_extension("tmp");
    let mut out = File::create(&tmp).map_err(|e| Error::io(&tmp.display().to_string(), e))?;
    out.write_all(&header)
        .map_err(|e| Error::io(&tmp.display().to_string(), e))?;
    out.write_all(&table)
        .map_err(|e| Error::io(&tmp.display().to_string(), e))?;
    write_zeros(&mut out, (data_off - table_end) as usize)?;
    let mut data_cursor = 0u64;
    for (p, off) in parts.iter().zip(&placed) {
        if *off > data_cursor {
            write_zeros(&mut out, (*off - data_cursor) as usize)?;
        }
        match p.src {
            Src::File {
                path: src,
                off: src_off,
            } => copy_range(src, src_off, p.len, &mut out)?,
            Src::Mem(bytes) => {
                if bytes.len() as u64 != p.len {
                    return Err(Error::Bad(format!("{}: memory length", p.name)));
                }
                out.write_all(bytes).map_err(|e| Error::Io(e.to_string()))?;
            }
        }
        data_cursor = off + p.len;
    }
    out.sync_all()
        .map_err(|e| Error::io(&tmp.display().to_string(), e))?;
    drop(out);
    std::fs::rename(&tmp, path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    Ok(())
}

fn write_zeros(out: &mut File, n: usize) -> Result<()> {
    if n == 0 {
        return Ok(());
    }
    let buf = vec![0u8; n.min(4096)];
    let mut left = n;
    while left > 0 {
        let k = left.min(buf.len());
        out.write_all(&buf[..k])
            .map_err(|e| Error::Io(e.to_string()))?;
        left -= k;
    }
    Ok(())
}

fn copy_range(src: &Path, off: u64, len: u64, out: &mut File) -> Result<()> {
    let mut input = File::open(src).map_err(|e| Error::io(&src.display().to_string(), e))?;
    input
        .seek(SeekFrom::Start(off))
        .map_err(|e| Error::io(&src.display().to_string(), e))?;
    let mut left = len;
    let mut buf = [0u8; 1024 * 1024];
    while left > 0 {
        let n = buf.len().min(left as usize);
        input
            .read_exact(&mut buf[..n])
            .map_err(|e| Error::io(&src.display().to_string(), e))?;
        out.write_all(&buf[..n])
            .map_err(|e| Error::Io(e.to_string()))?;
        left -= n as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_tensor::gq::{GqDType, GqFile, Prefault};

    #[test]
    fn mem_bytes_roundtrip_without_a_weight_file() {
        let dir = std::env::temp_dir().join(format!("galion-gq-mem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes: Vec<u8> = [0.5f32, -1.0]
            .into_iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let path = dir.join("voice_encoder.gq");
        write_gq_mem(
            &path,
            &[MemTensor {
                name: "sym.voice_encoder.encoder_weight".into(),
                dtype: GqDType::F32,
                dims: vec![2],
                bytes: bytes.clone(),
            }],
        )
        .unwrap();
        let gq = GqFile::open(&path, Prefault::None).unwrap();
        gq.verify_all().unwrap();
        assert_eq!(
            gq.raw("sym.voice_encoder.encoder_weight").unwrap(),
            bytes.as_slice()
        );
        assert_eq!(
            gq.f32_view("sym.voice_encoder.encoder_weight")
                .unwrap()
                .unwrap(),
            &[0.5f32, -1.0]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_range_roundtrip_matches_the_buffer() {
        let dir = std::env::temp_dir().join(format!("galion-gq-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes: Vec<u8> = [0.25f32, 2.0]
            .into_iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let src = dir.join("span.bin");
        let mut padded = vec![0xABu8; 8];
        padded.extend_from_slice(&bytes);
        std::fs::write(&src, &padded).unwrap();
        let path = dir.join("voice_encoder.gq");
        write_gq(
            &path,
            &[GqPart {
                name: "sym.voice_encoder.encoder_weight".into(),
                dtype: GqDType::F32,
                dims: vec![2],
                src,
                src_off: 8,
                len: bytes.len() as u64,
                sha256: crate::sha::sha256(&bytes),
            }],
        )
        .unwrap();
        let gq = GqFile::open(&path, Prefault::None).unwrap();
        gq.verify_all().unwrap();
        assert_eq!(
            gq.raw("sym.voice_encoder.encoder_weight").unwrap(),
            bytes.as_slice()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
