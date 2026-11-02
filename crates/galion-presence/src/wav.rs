// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Std-only RIFF/WAVE PCM loader for presence fixtures (A13, wave 2).
//!
//! Supported input (anything else is a clear error, never a guess):
//! * container: `RIFF` / `WAVE`, little-endian; unknown chunks (`LIST`, `fact`, …) are skipped;
//! * `fmt ` format tag `1` (PCM) or `0xFFFE` (WAVE_FORMAT_EXTENSIBLE with the PCM sub-format GUID);
//! * channels: 1 (mono) or 2 (stereo; downmixed to mono as (L+R)/2);
//! * bits per sample: 8 (unsigned), 16, 24 or 32 (signed integer);
//! * any sample rate from 8 000 to 192 000 Hz (an 80 ms frame must be a whole number of samples).
//!
//! Float WAV, compressed formats, >2 channels and RF64 are rejected.
//!
//! The loader turns audio into the harness' existing per-frame `you` states with a
//! deterministic RMS energy gate, so the replay path (and the Mock backend) are unchanged:
//! `speech` if frame RMS >= `speech_dbfs`, `breath` if >= `breath_dbfs`, else `silence`.

use crate::spec::YouState;

/// Decoded PCM, downmixed to mono, normalised to [-1, 1].
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub sample_rate: u32,
    /// Channels in the source file (1 or 2) before downmix.
    pub channels: u16,
    pub bits_per_sample: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    /// Samples in one frame of `frame_ms` milliseconds (errors if not integral).
    pub fn frame_len(&self, frame_ms: u32) -> Result<usize, String> {
        let num = self.sample_rate as u64 * frame_ms as u64;
        if num % 1000 != 0 {
            return Err(format!("wav: {} Hz does not divide into whole {frame_ms} ms frames", self.sample_rate));
        }
        Ok((num / 1000) as usize)
    }
    /// Number of frames (the last partial frame counts as a frame).
    pub fn frames(&self, frame_ms: u32) -> Result<u32, String> {
        let fl = self.frame_len(frame_ms)?;
        Ok(self.samples.len().div_ceil(fl) as u32)
    }
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// KSDATAFORMAT_SUBTYPE_PCM bytes 2..16 (after the 2-byte format code).
const PCM_GUID_TAIL: [u8; 14] = [0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71];

/// Parse a WAV file from memory.
pub fn parse_wav(b: &[u8]) -> Result<Pcm, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("wav: not a RIFF/WAVE file".into());
    }
    let mut i = 12;
    let mut fmt: Option<(u16, u32, u16)> = None; // channels, rate, bits
    let mut data: Option<&[u8]> = None;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32le(b, i + 4) as usize;
        let body = i + 8;
        let end = body.checked_add(len).ok_or("wav: chunk size overflow")?;
        // Tolerate a truncated final data chunk (common in streamed writers): clamp to EOF.
        let end_clamped = end.min(b.len());
        match id {
            b"fmt " => {
                if len < 16 || end > b.len() {
                    return Err("wav: fmt chunk too short".into());
                }
                let tag = u16le(b, body);
                let ch = u16le(b, body + 2);
                let rate = u32le(b, body + 4);
                let block_align = u16le(b, body + 12);
                let bits = u16le(b, body + 14);
                let pcm = match tag {
                    1 => true,
                    0xFFFE => len >= 40 && u16le(b, body + 24) == 1 && b[body + 26..body + 40] == PCM_GUID_TAIL,
                    _ => false,
                };
                if !pcm {
                    return Err(format!("wav: format tag 0x{tag:04x} is not integer PCM (float/compressed unsupported)"));
                }
                if !(1..=2).contains(&ch) {
                    return Err(format!("wav: {ch} channels unsupported (mono or stereo only)"));
                }
                if ![8, 16, 24, 32].contains(&bits) {
                    return Err(format!("wav: {bits}-bit PCM unsupported (8/16/24/32)"));
                }
                if !(8_000..=192_000).contains(&rate) {
                    return Err(format!("wav: sample rate {rate} Hz out of range 8000..=192000"));
                }
                if block_align as u32 != ch as u32 * bits as u32 / 8 {
                    return Err(format!("wav: block_align {block_align} != channels*bits/8"));
                }
                fmt = Some((ch, rate, bits));
            }
            b"data" => data = Some(&b[body.min(b.len())..end_clamped]),
            _ => {}
        }
        i = end + (len & 1); // chunks are word-aligned
    }
    let (channels, sample_rate, bits) = fmt.ok_or("wav: missing fmt chunk")?;
    let data = data.ok_or("wav: missing data chunk")?;
    let bps = bits as usize / 8;
    let frame = bps * channels as usize;
    let n = data.len() / frame;
    let mut samples = Vec::with_capacity(n);
    let read = |o: usize| -> f32 {
        match bits {
            8 => (data[o] as f32 - 128.0) / 128.0,
            16 => i16::from_le_bytes([data[o], data[o + 1]]) as f32 / 32_768.0,
            24 => (i32::from_le_bytes([0, data[o], data[o + 1], data[o + 2]]) >> 8) as f32 / 8_388_608.0,
            _ => i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as f32 / 2_147_483_648.0,
        }
    };
    for k in 0..n {
        let o = k * frame;
        let s = if channels == 2 { 0.5 * (read(o) + read(o + bps)) } else { read(o) };
        samples.push(s);
    }
    Ok(Pcm { sample_rate, channels, bits_per_sample: bits, samples })
}

pub fn read_wav(path: &std::path::Path) -> Result<Pcm, String> {
    let b = std::fs::read(path).map_err(|e| format!("wav: cannot read {}: {e}", path.display()))?;
    parse_wav(&b).map_err(|e| format!("{}: {e}", path.display()))
}

/// Encode interleaved i16 samples as a canonical 44-byte-header PCM WAV (fixtures, tests).
pub fn encode_pcm16(sample_rate: u32, channels: u16, interleaved: &[i16]) -> Vec<u8> {
    let data_len = (interleaved.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(channels * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in interleaved {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// Energy-gate thresholds in dBFS (RMS, full-scale sine = -3 dBFS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gate {
    pub speech_dbfs: f32,
    pub breath_dbfs: f32,
}

impl Default for Gate {
    fn default() -> Self {
        Gate { speech_dbfs: -35.0, breath_dbfs: -55.0 }
    }
}

/// RMS level of one frame in dBFS (silence floors at -120).
pub fn rms_dbfs(x: &[f32]) -> f32 {
    if x.is_empty() {
        return -120.0;
    }
    let e: f64 = x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len() as f64;
    if e <= 1e-12 {
        -120.0
    } else {
        (10.0 * e.log10()) as f32
    }
}

/// Per-frame you-states from PCM. A trailing partial frame is measured on what it has.
pub fn you_states(pcm: &Pcm, frame_ms: u32, gate: Gate) -> Result<Vec<YouState>, String> {
    let fl = pcm.frame_len(frame_ms)?;
    Ok(pcm
        .samples
        .chunks(fl)
        .map(|c| {
            let db = rms_dbfs(c);
            if db >= gate.speech_dbfs {
                YouState::Speech
            } else if db >= gate.breath_dbfs {
                YouState::Breath
            } else {
                YouState::Silence
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_mono_and_stereo_pcm16() {
        let mono = encode_pcm16(16_000, 1, &[0, 16_384, -16_384, 32_767]);
        let p = parse_wav(&mono).unwrap();
        assert_eq!((p.sample_rate, p.channels, p.bits_per_sample), (16_000, 1, 16));
        assert_eq!(p.samples, vec![0.0, 0.5, -0.5, 32_767.0 / 32_768.0]);
        let st = encode_pcm16(8_000, 2, &[16_384, 0, -16_384, -16_384]);
        let p = parse_wav(&st).unwrap();
        assert_eq!(p.channels, 2);
        assert_eq!(p.samples, vec![0.25, -0.5]);
        assert_eq!(p.frame_len(80).unwrap(), 640);
    }

    #[test]
    fn rejects_unsupported() {
        let mut f = encode_pcm16(16_000, 1, &[0; 4]);
        f[20] = 3; // IEEE float tag
        assert!(parse_wav(&f).unwrap_err().contains("not integer PCM"));
        assert!(parse_wav(b"RIFX0000WAVE").is_err());
        let three = encode_pcm16(16_000, 3, &[0; 6]);
        assert!(parse_wav(&three).unwrap_err().contains("channels"));
        let odd = encode_pcm16(11_111, 1, &[0; 4]);
        assert!(parse_wav(&odd).unwrap().frame_len(80).is_err());
    }
}
