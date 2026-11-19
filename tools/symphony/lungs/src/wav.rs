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
    let audio_format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as usize;
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if channels == 0 || channels > 2 {
        return Err("only mono and stereo references are read".into());
    }
    if sample_rate == 0 {
        return Err("sample rate is 0".into());
    }
    let frame = match (audio_format, bits) {
        (1, 16) => 2 * channels,
        (3, 32) => 4 * channels,
        _ => {
            return Err(format!(
                "unsupported WAV format {audio_format} at {bits} bits"
            ))
        }
    };
    if data.len() % frame != 0 {
        return Err("data chunk is not a whole number of frames".into());
    }
    let mut samples = Vec::with_capacity(data.len() / frame);
    for frame_b in data.chunks(frame) {
        let mono = if audio_format == 1 {
            let left = i16::from_le_bytes([frame_b[0], frame_b[1]]) as f64 / 32768.0;
            if channels == 2 {
                let right = i16::from_le_bytes([frame_b[2], frame_b[3]]) as f64 / 32768.0;
                0.5 * (left + right)
            } else {
                left
            }
        } else {
            let left = f32::from_le_bytes([frame_b[0], frame_b[1], frame_b[2], frame_b[3]]) as f64;
            if channels == 2 {
                let right =
                    f32::from_le_bytes([frame_b[4], frame_b[5], frame_b[6], frame_b[7]]) as f64;
                0.5 * (left + right)
            } else {
                left
            }
        };
        samples.push(mono);
    }
    Ok(Wav {
        sample_rate: sample_rate as f64,
        samples,
    })
}
