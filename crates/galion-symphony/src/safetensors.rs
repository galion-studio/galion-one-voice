// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Safetensors is an interchange file. It is read here and not kept as the
//! symphony runtime. Only F32 and BF16 are accepted. Other dtypes are refused
//! rather than cast.

use crate::json;
use crate::sha::hash_range;
use crate::{Error, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_HEADER: u64 = 100 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StDtype {
    F32,
    BF16,
}

impl StDtype {
    pub fn from_name(s: &str) -> Result<Self> {
        match s {
            "F32" => Ok(StDtype::F32),
            "BF16" => Ok(StDtype::BF16),
            other => Err(Error::Refused(format!(
                "safetensors dtype {other} is not a Galion symphony dtype (F32 or BF16 only; no silent cast)"
            ))),
        }
    }

    pub fn size(self) -> u64 {
        match self {
            StDtype::F32 => 4,
            StDtype::BF16 => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StTensor {
    pub name: String,
    pub dtype: StDtype,
    pub shape: Vec<u64>,
    /// Absolute file offset of the payload.
    pub file_off: u64,
    pub len: u64,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StFile {
    pub metadata_format: Option<String>,
    pub tensors: Vec<StTensor>,
}

/// True when the first four bytes are a foreign packed-weight magic.
pub fn foreign_weight_magic(b: &[u8]) -> bool {
    foreign4(b, b'g', b'g', b'u', b'f') || foreign4(b, b'g', b'g', b'm', b'l')
}

fn foreign4(b: &[u8], a: u8, c: u8, d: u8, e: u8) -> bool {
    b.len() >= 4
        && b[0].eq_ignore_ascii_case(&a)
        && b[1].eq_ignore_ascii_case(&c)
        && b[2].eq_ignore_ascii_case(&d)
        && b[3].eq_ignore_ascii_case(&e)
}

/// File extension this stack will not read, even if renamed later fails closed.
pub fn refused_extension(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    matches!(e.as_str(), "pt" | "pth" | "bin" | "ckpt" | "onnx" | "ggml")
        || foreign_ext(e.as_bytes())
}

fn foreign_ext(b: &[u8]) -> bool {
    b.len() == 4 && b[0] == b'g' && b[1] == b'g' && b[2] == b'u' && b[3] == b'f'
}

pub fn read_safetensors(path: &Path) -> Result<StFile> {
    let mut file = File::open(path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    let file_len = file
        .metadata()
        .map_err(|e| Error::io(&path.display().to_string(), e))?
        .len();
    let mut prefix = [0u8; 8];
    file.read_exact(&mut prefix)
        .map_err(|e| Error::io(&path.display().to_string(), e))?;
    if foreign_weight_magic(&prefix) {
        return Err(Error::Refused(format!(
            "{} starts with a foreign weight magic; symphony loads .gq only",
            path.display()
        )));
    }
    let header_len = u64::from_le_bytes(prefix);
    if header_len == 0 || header_len > MAX_HEADER || 8 + header_len > file_len {
        return Err(Error::Bad(format!(
            "{}: safetensors header length",
            path.display()
        )));
    }
    let mut header = vec![0u8; header_len as usize];
    file.read_exact(&mut header)
        .map_err(|e| Error::io(&path.display().to_string(), e))?;
    let header = std::str::from_utf8(&header)
        .map_err(|_| Error::Bad(format!("{}: header is not utf-8", path.display())))?;
    let header = header.trim_end();
    let value = json::parse(header).map_err(|e| Error::Bad(format!("{}: {e}", path.display())))?;
    let obj = value.as_obj()?;
    let data_base = 8 + header_len;
    let data_len = file_len - data_base;
    let mut metadata_format = None;
    let mut tensors = Vec::new();
    let mut spans: Vec<(u64, u64, String)> = Vec::new();
    for (key, val) in obj {
        if key == "__metadata__" {
            if let Some(fmt) = val.get("format").and_then(|v| v.as_str().ok()) {
                metadata_format = Some(fmt.to_owned());
            }
            continue;
        }
        if key.is_empty() || key.contains(['\n', '\r']) {
            return Err(Error::Bad(format!("{}: bad tensor name", path.display())));
        }
        let dtype = StDtype::from_name(
            val.get("dtype")
                .and_then(|v| v.as_str().ok())
                .ok_or_else(|| Error::Bad(format!("{}: {key} missing dtype", path.display())))?,
        )?;
        let shape_j = val
            .get("shape")
            .ok_or_else(|| Error::Bad(format!("{}: {key} missing shape", path.display())))?;
        let mut shape = Vec::new();
        for d in shape_j.as_arr()? {
            shape.push(d.as_u64()?);
        }
        if shape.len() > galion_tensor::gq::MAX_RANK {
            return Err(Error::Refused(format!(
                "{}: {key} rank {} exceeds Galion max",
                path.display(),
                shape.len()
            )));
        }
        let offs = val
            .get("data_offsets")
            .ok_or_else(|| Error::Bad(format!("{}: {key} missing data_offsets", path.display())))?
            .as_arr()?;
        if offs.len() != 2 {
            return Err(Error::Bad(format!(
                "{}: {key} data_offsets",
                path.display()
            )));
        }
        let start = offs[0].as_u64()?;
        let end = offs[1].as_u64()?;
        if end < start || end > data_len {
            return Err(Error::Bad(format!(
                "{}: {key} offsets out of range",
                path.display()
            )));
        }
        let len = end - start;
        let numel = shape
            .iter()
            .try_fold(1u64, |a, &d| a.checked_mul(d))
            .ok_or_else(|| Error::Bad(format!("{key}: shape overflow")))?;
        let expect = numel
            .checked_mul(dtype.size())
            .ok_or_else(|| Error::Bad(format!("{key}: size overflow")))?;
        if len != expect {
            return Err(Error::Bad(format!(
                "{}: {key} payload {len} bytes, dtype wants {expect}",
                path.display()
            )));
        }
        let file_off = data_base + start;
        let sha256 = hash_range(&mut file, file_off, len)
            .map_err(|e| Error::io(&path.display().to_string(), e))?;
        spans.push((start, end, key.clone()));
        tensors.push(StTensor {
            name: key.clone(),
            dtype,
            shape,
            file_off,
            len,
            sha256,
        });
    }
    if tensors.is_empty() {
        return Err(Error::Bad(format!("{}: no tensors", path.display())));
    }
    spans.sort_unstable();
    for w in spans.windows(2) {
        if w[1].0 < w[0].1 {
            return Err(Error::Bad(format!(
                "{}: tensor overlap {}",
                path.display(),
                w[1].2
            )));
        }
    }
    Ok(StFile {
        metadata_format,
        tensors,
    })
}

/// Write a tiny safetensors file. Fixture and tests only.
pub fn write_safetensors(path: &Path, tensors: &[(&str, StDtype, &[u64], &[u8])]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(&parent.display().to_string(), e))?;
    }
    let mut names: Vec<usize> = (0..tensors.len()).collect();
    names.sort_by(|&a, &b| tensors[a].0.cmp(tensors[b].0));
    let mut json =
        String::from("{\"__metadata__\":{\"format\":\"pt\",\"note\":\"galion-fixture\"}");
    let mut data = Vec::new();
    for &i in &names {
        let (name, dtype, shape, bytes) = tensors[i];
        let b = data.len();
        data.extend_from_slice(bytes);
        let dt = match dtype {
            StDtype::F32 => "F32",
            StDtype::BF16 => "BF16",
        };
        let shape_s = shape
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        json.push_str(&format!(
            ",\"{name}\":{{\"dtype\":\"{dt}\",\"shape\":[{shape_s}],\"data_offsets\":[{b},{}]}}",
            data.len()
        ));
    }
    json.push('}');
    while (8 + json.len()) % 8 != 0 {
        json.push(' ');
    }
    let mut out = (json.len() as u64).to_le_bytes().to_vec();
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(&data);
    std::fs::write(path, out).map_err(|e| Error::io(&path.display().to_string(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_fixture_and_refuse_magic() {
        let dir = std::env::temp_dir().join(format!("galion-st-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ve.safetensors");
        let payload = 1.5f32.to_le_bytes();
        write_safetensors(&path, &[("encoder.weight", StDtype::F32, &[1], &payload)]).unwrap();
        let st = read_safetensors(&path).unwrap();
        assert_eq!(st.metadata_format.as_deref(), Some("pt"));
        assert_eq!(st.tensors[0].name, "encoder.weight");
        assert_eq!(st.tensors[0].sha256, crate::sha::sha256(&payload));
        let bad = dir.join("foreign.safetensors");
        let mut magic = vec![0u8; 16];
        magic[0] = b'G';
        magic[1] = b'G';
        magic[2] = b'U';
        magic[3] = b'F';
        std::fs::write(&bad, &magic).unwrap();
        assert!(matches!(read_safetensors(&bad), Err(Error::Refused(_))));
        assert!(refused_extension("pt"));
        assert!(refused_extension("GGML"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
