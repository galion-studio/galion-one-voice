// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! WAV facts for the clip ABI. This reader does not downmix, resample, or drop chunks.
//! `galion-presence` downmixes stereo for the harness. A reference clip must not.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavFacts {
    pub audio_format: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits: u16,
    pub data_bytes: u64,
    pub duration_ms: u64,
}

pub fn facts(b: &[u8]) -> Result<WavFacts> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(Error::Bad("wav: not RIFF/WAVE".into()));
    }
    let mut i = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data_bytes = None;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = i + 8;
        let end = body
            .checked_add(len)
            .ok_or_else(|| Error::Bad("wav: chunk overflow".into()))?;
        if end > b.len() {
            return Err(Error::Bad("wav: chunk past end".into()));
        }
        if id == b"fmt " {
            if len < 16 {
                return Err(Error::Bad("wav: fmt too short".into()));
            }
            let tag = u16::from_le_bytes([b[body], b[body + 1]]);
            let ch = u16::from_le_bytes([b[body + 2], b[body + 3]]);
            let rate = u32::from_le_bytes([b[body + 4], b[body + 5], b[body + 6], b[body + 7]]);
            let bits = u16::from_le_bytes([b[body + 14], b[body + 15]]);
            fmt = Some((tag, ch, rate, bits));
        } else if id == b"data" {
            data_bytes = Some(len as u64);
        }
        let mut next = end;
        if len % 2 == 1 {
            next += 1;
        }
        if next > b.len() {
            break;
        }
        i = next;
    }
    let Some((tag, channels, rate, bits)) = fmt else {
        return Err(Error::Bad("wav: no fmt chunk".into()));
    };
    let Some(data_bytes) = data_bytes else {
        return Err(Error::Bad("wav: no data chunk".into()));
    };
    if tag != 1 {
        return Err(Error::Refused(
            "wav: only PCM format 1 is a clip payload".into(),
        ));
    }
    if !(1..=2).contains(&channels) || bits != 16 || !(8_000..=192_000).contains(&rate) {
        return Err(Error::Refused(format!(
            "wav: refuse {channels} ch, {bits} bit, {rate} Hz"
        )));
    }
    let frame = channels as u64 * 2;
    if frame == 0 || data_bytes % frame != 0 {
        return Err(Error::Bad("wav: data length is not a whole frame".into()));
    }
    let frames = data_bytes / frame;
    let duration_ms = frames.saturating_mul(1000) / rate as u64;
    Ok(WavFacts {
        audio_format: tag,
        channels,
        sample_rate: rate,
        bits,
        data_bytes,
        duration_ms,
    })
}

/// Interleaved i16, one slot per channel. No average, no drop.
pub fn interleaved_i16(b: &[u8]) -> Result<Vec<i16>> {
    let facts = facts(b)?;
    let data = data_chunk(b)?;
    if data.len() as u64 != facts.data_bytes {
        return Err(Error::Bad("wav: data chunk length".into()));
    }
    let mut out = Vec::with_capacity(data.len() / 2);
    for c in data.chunks_exact(2) {
        out.push(i16::from_le_bytes([c[0], c[1]]));
    }
    Ok(out)
}

fn data_chunk(b: &[u8]) -> Result<&[u8]> {
    let mut i = 12usize;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = i + 8;
        let end = body + len;
        if end > b.len() {
            return Err(Error::Bad("wav: chunk past end".into()));
        }
        if id == b"data" {
            return Ok(&b[body..end]);
        }
        i = end + (len % 2);
    }
    Err(Error::Bad("wav: no data chunk".into()))
}

pub fn write_pcm16(
    path: &std::path::Path,
    rate: u32,
    channels: u16,
    samples: &[i16],
    extra: &[([u8; 4], &[u8])],
) -> Result<()> {
    if channels == 0 || samples.len() % channels as usize != 0 {
        return Err(Error::Bad("wav: sample count".into()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(&parent.display().to_string(), e))?;
    }
    let data_bytes = samples.len() * 2;
    let mut extra_bytes = 0usize;
    for (id, body) in extra {
        if id.iter().any(|c| !c.is_ascii()) {
            return Err(Error::Bad("wav: chunk id".into()));
        }
        extra_bytes += 8 + body.len() + (body.len() % 2);
    }
    let riff_size = 4 + (8 + 16) + (8 + data_bytes) + extra_bytes;
    let mut out = Vec::with_capacity(8 + riff_size);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_size as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    let byte_rate = rate * channels as u32 * 2;
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    for (id, body) in extra {
        out.extend_from_slice(id);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
    }
    std::fs::write(path, out).map_err(|e| Error::io(&path.display().to_string(), e))
}
