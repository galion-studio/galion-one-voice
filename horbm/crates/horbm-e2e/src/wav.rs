// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! 16-bit mono WAV. Written only under a caller-supplied temp path.

use std::fs;
use std::path::Path;

pub fn write_wav(path: &Path, rate: u32, samples: &[i16]) -> Result<(), String> {
    if rate == 0 {
        return Err("wav rate is 0".into());
    }
    let data_len = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| "wav is too long".to_string())?;
    let riff_len = 36u32
        .checked_add(u32::try_from(data_len).map_err(|_| "wav is too long".to_string())?)
        .ok_or_else(|| "wav is too long".to_string())?;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_len.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    let byte_rate = rate.saturating_mul(2);
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).map_err(|err| format!("write {}: {err}", path.display()))
}

pub fn read_wav(path: &Path) -> Result<(u32, Vec<i16>), String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    parse_wav(&bytes).map_err(|err| format!("{}: {err}", path.display()))
}

fn parse_wav(bytes: &[u8]) -> Result<(u32, Vec<i16>), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not RIFF/WAVE".into());
    }
    let mut index = 12usize;
    let mut rate = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    while index + 8 <= bytes.len() {
        let id = &bytes[index..index + 4];
        let size =
            u32::from_le_bytes(bytes[index + 4..index + 8].try_into().unwrap_or([0; 4])) as usize;
        let end = index
            .saturating_add(8)
            .saturating_add(size)
            .min(bytes.len());
        let body = &bytes[index + 8..end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err("short fmt chunk".into());
            }
            channels = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            if bits != 16 || channels != 1 || rate == 0 {
                return Err("want 16-bit mono".into());
            }
            let samples = body
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            return Ok((rate, samples));
        }
        index = index.saturating_add(8).saturating_add(size);
        if size % 2 == 1 {
            index = index.saturating_add(1);
        }
    }
    Err("no data chunk".into())
}

pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|sample| {
            let finite = if sample.is_finite() { *sample } else { 0.0 };
            (finite.clamp(-1.0, 1.0) * 32_767.0).round() as i16
        })
        .collect()
}

pub fn resample_i16(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() || from == to {
        return input.to_vec();
    }
    let n = (input.len() as u64).saturating_mul(u64::from(to)) / u64::from(from);
    let step = f64::from(from) / f64::from(to);
    (0..n)
        .map(|index| {
            let pos = index as f64 * step;
            let left = pos as usize;
            let frac = pos - left as f64;
            let a = f64::from(input[left.min(input.len() - 1)]);
            let b = f64::from(input[(left + 1).min(input.len() - 1)]);
            (a + (b - a) * frac).round().clamp(-32_768.0, 32_767.0) as i16
        })
        .collect()
}

/// Bring a quiet synth up so the absolute energy gate can hear it.
/// Silence stays silence. A peak already at or above 4000 is left alone.
pub fn lift_quiet(samples: &mut [i16]) {
    let peak = samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    if peak == 0 || peak >= 4_000 {
        return;
    }
    let gain = 8_000.0 / f32::from(peak);
    for sample in samples.iter_mut() {
        let scaled = f32::from(*sample) * gain;
        *sample = scaled.round().clamp(-32_767.0, 32_767.0) as i16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn a_temp_wav_roundtrips_and_is_removed() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("horbm-e2e-wav-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round.wav");
        let samples = vec![0, 1000, -1000, 50];
        write_wav(&path, 16_000, &samples).unwrap();
        let (rate, back) = read_wav(&path).unwrap();
        assert_eq!(rate, 16_000);
        assert_eq!(back, samples);
        fs::remove_dir_all(&dir).unwrap();
        assert!(!path.exists());
    }
}
