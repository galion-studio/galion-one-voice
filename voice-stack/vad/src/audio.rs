// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! WAV in and linear resample. The byte layout and the resample formula match
//! `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` so a 20 ms frame from that lane is the
//! same frame here. No codec crate: CC0 clips used by the bench are 16-bit PCM.

use crate::VadError;

#[derive(Debug, Clone)]
pub struct Wav {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

pub fn read_wav(path: &std::path::Path) -> Result<Wav, VadError> {
    let b = std::fs::read(path).map_err(|e| VadError::new(format!("{}: {e}", path.display())))?;
    parse_wav(&b).map_err(|e| VadError::new(format!("{}: {e}", path.display())))
}

pub fn parse_wav(b: &[u8]) -> Result<Wav, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not RIFF/WAVE".into());
    }
    let (mut i, mut rate, mut ch, mut bits) = (12usize, 0u32, 0u16, 0u16);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let sz = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let end = (i + 8 + sz).min(b.len());
        let body = &b[i + 8..end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err("short fmt chunk".into());
            }
            ch = u16::from_le_bytes(body[2..4].try_into().unwrap());
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            if bits != 16 {
                return Err(format!("{bits}-bit PCM, want 16"));
            }
            if ch == 0 || rate == 0 {
                return Err("fmt chunk missing".into());
            }
            let samples = body
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c))
                .collect();
            return Ok(Wav {
                rate,
                channels: ch,
                samples,
            });
        }
        i += 8 + sz + (sz & 1);
    }
    Err("no data chunk".into())
}

pub fn to_mono(w: &Wav) -> Vec<i16> {
    if w.channels <= 1 {
        return w.samples.clone();
    }
    let c = w.channels as usize;
    w.samples
        .chunks_exact(c)
        .map(|f| (f.iter().map(|&s| s as i32).sum::<i32>() / c as i32) as i16)
        .collect()
}

/// Linear resample. Same formula as `galion_v2_live::audio::resample`.
pub fn resample(x: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == to || x.is_empty() || from == 0 || to == 0 {
        return x.to_vec();
    }
    let n = (x.len() as u64 * to as u64 / from as u64) as usize;
    let step = from as f64 / to as f64;
    (0..n)
        .map(|i| {
            let p = i as f64 * step;
            let j = p as usize;
            let fr = p - j as f64;
            let a = x[j.min(x.len() - 1)] as f64;
            let b = x[(j + 1).min(x.len() - 1)] as f64;
            (a + (b - a) * fr).round() as i16
        })
        .collect()
}

/// dBFS of an s16 frame. Same formula as `galion_v2_live::audio::rms_db`.
pub fn rms_db(x: &[i16]) -> f64 {
    if x.is_empty() {
        return -120.0;
    }
    let e = x.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum::<f64>() / x.len() as f64;
    10.0 * e.max(1e-12).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_minus_120_db() {
        assert_eq!(rms_db(&[]), -120.0);
        assert!((rms_db(&[0, 0, 0, 0]) - -120.0).abs() < 1e-9);
    }

    #[test]
    fn constant_24k_frame_resamples_to_320() {
        let x = vec![1000i16; 480];
        let y = resample(&x, 24_000, 16_000);
        assert_eq!(y.len(), 320);
        assert!(y.iter().all(|&s| s == 1000));
    }

    #[test]
    fn wav_roundtrip_and_stereo_downmix() {
        let samples = vec![0i16, 1000, -1000, 500];
        let mut bytes = Vec::new();
        write_pcm16(&mut bytes, 16_000, 1, &samples);
        let w = parse_wav(&bytes).unwrap();
        assert_eq!(w.rate, 16_000);
        assert_eq!(w.samples, samples);
        let stereo = Wav {
            rate: 16_000,
            channels: 2,
            samples: vec![1000, -1000, 2000, 0],
        };
        assert_eq!(to_mono(&stereo), vec![0, 1000]);
    }

    fn write_pcm16(out: &mut Vec<u8>, rate: u32, channels: u16, s: &[i16]) {
        let data = (s.len() * 2) as u32;
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for v in s {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}
