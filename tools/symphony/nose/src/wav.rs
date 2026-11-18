// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Minimal WAV read and write for a speaker reference at runtime.
//! Integer PCM 16 and IEEE float 32, mono or stereo (down-mixed).
//! Nothing here stores a recording into the repository.

use std::fs::File;
use std::io::Write;
use std::path::Path;

const MAX_BYTES: usize = 64 * 1024 * 1024;

pub struct Wav {
    pub sample_rate: f64,
    pub samples: Vec<f64>,
}

pub fn read_wav(path: &Path) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > MAX_BYTES {
        return Err(format!("{} is over 64 MB", path.display()));
    }
    parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn write_wav_mono_i16(path: &Path, sample_rate: u32, samples: &[f64]) -> Result<(), String> {
    if sample_rate == 0 {
        return Err("sample rate is 0".into());
    }
    let mut data = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        data.extend_from_slice(&q.to_le_bytes());
    }
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    let mut f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(&out)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

fn parse(b: &[u8]) -> Result<Wav, String> {
    if b.len() < 44 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a RIFF WAVE file".into());
    }
    let mut fmt = None;
    let mut data = None;
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let start = i + 8;
        let end = start.saturating_add(size);
        if end > b.len() {
            return Err("chunk runs past the end of the file".into());
        }
        if id == b"fmt " {
            fmt = Some(&b[start..end]);
        } else if id == b"data" {
            data = Some(&b[start..end]);
        }
        i = end + (size % 2);
    }
    let fmt = fmt.ok_or("missing fmt chunk")?;
    let data = data.ok_or("missing data chunk")?;
    if fmt.len() < 16 {
        return Err("fmt chunk is short".into());
    }
    let format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as usize;
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if channels == 0 || sample_rate == 0 {
        return Err("fmt has no channels or no sample rate".into());
    }
    let samples = match (format, bits) {
        (1, 16) => decode_i16(data, channels)?,
        (3, 32) => decode_f32(data, channels)?,
        _ => return Err(format!("unsupported WAV format {format} at {bits} bits")),
    };
    Ok(Wav {
        sample_rate: sample_rate as f64,
        samples,
    })
}

fn decode_i16(data: &[u8], channels: usize) -> Result<Vec<f64>, String> {
    let frame = channels * 2;
    if data.len() % frame != 0 {
        return Err("pcm16 data is not an even number of frames".into());
    }
    let mut out = Vec::with_capacity(data.len() / frame);
    for frame_b in data.chunks(frame) {
        let mut acc = 0.0;
        for ch in 0..channels {
            let i = ch * 2;
            let s = i16::from_le_bytes([frame_b[i], frame_b[i + 1]]);
            acc += s as f64 / 32768.0;
        }
        out.push(acc / channels as f64);
    }
    Ok(out)
}

fn decode_f32(data: &[u8], channels: usize) -> Result<Vec<f64>, String> {
    let frame = channels * 4;
    if data.len() % frame != 0 {
        return Err("float data is not an even number of frames".into());
    }
    let mut out = Vec::with_capacity(data.len() / frame);
    for frame_b in data.chunks(frame) {
        let mut acc = 0.0;
        for ch in 0..channels {
            let i = ch * 4;
            let s =
                f32::from_le_bytes([frame_b[i], frame_b[i + 1], frame_b[i + 2], frame_b[i + 3]]);
            acc += s as f64;
        }
        out.push(acc / channels as f64);
    }
    Ok(out)
}
