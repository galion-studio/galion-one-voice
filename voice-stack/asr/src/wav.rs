// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::Path;

use crate::error::AsrError;
use crate::event::SAMPLE_RATE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmWav {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved 16-bit PCM.
    pub samples: Vec<i16>,
}

pub fn read_wav(path: &Path) -> Result<PcmWav, AsrError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AsrError::new(format!("cannot read {}: {e}", path.display())))?;
    read_wav_bytes(&bytes)
        .map_err(|e| AsrError::new(format!("{}: {}", path.display(), e.message())))
}

pub fn read_wav_bytes(data: &[u8]) -> Result<PcmWav, AsrError> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(AsrError::new("not a RIFF WAVE file"));
    }
    let mut i = 12usize;
    let mut format = None;
    let mut channels = None;
    let mut rate = None;
    let mut bits = None;
    let mut pcm = None;
    while i + 8 <= data.len() {
        let id = &data[i..i + 4];
        let size = u32::from_le_bytes(data[i + 4..i + 8].try_into().unwrap()) as usize;
        let start = i + 8;
        let end = start
            .checked_add(size)
            .ok_or_else(|| AsrError::new("wav chunk size overflow"))?;
        if end > data.len() {
            return Err(AsrError::new("wav chunk runs past the end of the file"));
        }
        let body = &data[start..end];
        if id == b"fmt " {
            if body.len() < 16 {
                return Err(AsrError::new("wav fmt chunk is short"));
            }
            format = Some(u16::from_le_bytes([body[0], body[1]]));
            channels = Some(u16::from_le_bytes([body[2], body[3]]));
            rate = Some(u32::from_le_bytes([body[4], body[5], body[6], body[7]]));
            bits = Some(u16::from_le_bytes([body[14], body[15]]));
        } else if id == b"data" {
            pcm = Some(body.to_vec());
        }
        let pad = size % 2;
        i = end + pad;
    }
    let format = format.ok_or_else(|| AsrError::new("wav has no fmt chunk"))?;
    let channels = channels.ok_or_else(|| AsrError::new("wav has no channel count"))?;
    let rate = rate.ok_or_else(|| AsrError::new("wav has no sample rate"))?;
    let bits = bits.ok_or_else(|| AsrError::new("wav has no bit depth"))?;
    let pcm = pcm.ok_or_else(|| AsrError::new("wav has no data chunk"))?;
    if format != 1 {
        return Err(AsrError::new(format!(
            "wav format {format} is not 16-bit PCM (format 1). No transcript was invented."
        )));
    }
    if bits != 16 {
        return Err(AsrError::new(format!(
            "wav is {bits}-bit, want 16-bit PCM. No transcript was invented."
        )));
    }
    if channels == 0 || rate == 0 {
        return Err(AsrError::new("wav channels or sample rate is zero"));
    }
    let frame = 2 * channels as usize;
    if pcm.len() % frame != 0 {
        return Err(AsrError::new(
            "wav data length is not a whole number of frames",
        ));
    }
    let mut samples = Vec::with_capacity(pcm.len() / 2);
    for chunk in pcm.chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(PcmWav {
        sample_rate: rate,
        channels,
        samples,
    })
}

pub fn wav_to_16k_mono(wav: &PcmWav) -> Result<Vec<i16>, AsrError> {
    let mono = downmix(wav);
    resample_linear(&mono, wav.sample_rate, SAMPLE_RATE)
}

fn downmix(wav: &PcmWav) -> Vec<i16> {
    let ch = wav.channels as usize;
    if ch == 1 {
        return wav.samples.clone();
    }
    wav.samples
        .chunks(ch)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|s| i32::from(*s)).sum();
            (sum / ch as i32) as i16
        })
        .collect()
}

pub fn resample_linear(input: &[i16], from: u32, to: u32) -> Result<Vec<i16>, AsrError> {
    if from == 0 || to == 0 {
        return Err(AsrError::new("resample rate is zero"));
    }
    if from == to || input.is_empty() {
        return Ok(input.to_vec());
    }
    let out_len = (input.len() as u128).saturating_mul(u128::from(to)) / u128::from(from);
    if out_len == 0 {
        return Ok(Vec::new());
    }
    if out_len > usize::MAX as u128 {
        return Err(AsrError::new("resampled audio is too long"));
    }
    let out_len = out_len as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = (i as f64) * (from as f64) / (to as f64);
        let i0 = (src.floor() as usize).min(last);
        let i1 = (i0 + 1).min(last);
        let frac = src - (i0 as f64);
        let s = input[i0] as f64 + (input[i1] as f64 - input[i0] as f64) * frac;
        let s = s.round().clamp(i16::MIN as f64, i16::MAX as f64);
        out.push(s as i16);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_bytes(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let mut data = Vec::new();
        for s in samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        let byte_rate = rate * u32::from(channels) * 2;
        fmt.extend_from_slice(&byte_rate.to_le_bytes());
        let block = channels * 2;
        fmt.extend_from_slice(&block.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        let riff_size = (4 + 8 + fmt.len() + 8 + data.len()) as u32;
        out.extend_from_slice(&riff_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend_from_slice(&fmt);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn reads_mono_pcm_and_downmixes_stereo() {
        let bytes = wav_bytes(&[1, -2, 3, -4], 16_000, 1);
        let wav = read_wav_bytes(&bytes).unwrap();
        assert_eq!(wav.samples, vec![1, -2, 3, -4]);
        let stereo = wav_bytes(&[0, 1000, -2, -4], 16_000, 2);
        let wav = read_wav_bytes(&stereo).unwrap();
        let mono = wav_to_16k_mono(&wav).unwrap();
        assert_eq!(mono, vec![500, -3]);
    }

    #[test]
    fn linear_resample_doubles_8k_to_16k() {
        let input = vec![0, 1000, 2000, 3000];
        let out = resample_linear(&input, 8_000, 16_000).unwrap();
        assert_eq!(out, vec![0, 500, 1000, 1500, 2000, 2500, 3000, 3000]);
    }
}
