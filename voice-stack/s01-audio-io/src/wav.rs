// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! PCM16 WAV read and write. Same layout as `crates/galion-v2-live/src/audio.rs`:
//! RIFF/WAVE, `fmt ` then `data`, 16-bit little-endian. No other codec.
//!
//! This module does not know about a voice. Callers pass samples they already
//! have. Nothing here reads `GALION_SAMANTHA_REF`.

use crate::AudioError;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmWav {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

pub fn read_wav(path: &Path) -> Result<PcmWav, AudioError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|err| AudioError::Wav(format!("{}: {err}", path.display())))?;
    parse_wav(&bytes).map_err(|err| AudioError::Wav(format!("{}: {err}", path.display())))
}

pub fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) -> Result<(), AudioError> {
    if rate == 0 || channels == 0 {
        return Err(AudioError::Rate);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| AudioError::Wav(format!("{}: {err}", parent.display())))?;
        }
    }
    let mut file =
        File::create(path).map_err(|err| AudioError::Wav(format!("{}: {err}", path.display())))?;
    write_wav_bytes(&mut file, rate, channels, samples)
        .map_err(|err| AudioError::Wav(format!("{}: {err}", path.display())))
}

fn write_wav_bytes(
    file: &mut File,
    rate: u32,
    channels: u16,
    samples: &[i16],
) -> std::io::Result<()> {
    let data_bytes = (samples.len() * 2) as u32;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * u32::from(channels) * 2).to_le_bytes())?;
    file.write_all(&(channels * 2).to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}

fn parse_wav(bytes: &[u8]) -> Result<PcmWav, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not RIFF/WAVE".into());
    }
    let mut i = 12usize;
    let mut rate = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut format = 0u16;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let size = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        let start = i + 8;
        let end = (start + size).min(bytes.len());
        let body = &bytes[start..end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err("short fmt chunk".into());
            }
            format = u16::from_le_bytes(body[0..2].try_into().unwrap());
            channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            if format != 1 {
                return Err(format!("format {format}, want PCM 1"));
            }
            if bits != 16 {
                return Err(format!("{bits}-bit, want 16"));
            }
            if channels == 0 || rate == 0 {
                return Err("fmt chunk missing".into());
            }
            let samples = body
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            return Ok(PcmWav {
                rate,
                channels,
                samples,
            });
        }
        i += 8 + size + (size & 1);
    }
    Err("no data chunk".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_pcm16() {
        let dir = std::env::temp_dir().join(format!("galion-audio-io-wav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let samples: Vec<i16> = (0..320)
            .map(|i| if i % 2 == 0 { 1000 } else { -1000 })
            .collect();
        write_wav(&path, 16_000, 1, &samples).unwrap();
        let read = read_wav(&path).unwrap();
        assert_eq!(read.rate, 16_000);
        assert_eq!(read.channels, 1);
        assert_eq!(read.samples, samples);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
