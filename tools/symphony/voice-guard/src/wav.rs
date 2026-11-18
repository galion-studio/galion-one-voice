// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! PCM WAV in, mono f32 out. No resample.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct Wav {
    pub interleaved: Vec<f32>,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

pub fn read_wav(path: &Path) -> Result<Wav, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = r.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(format!("{}: empty format", path.display()));
    }
    let interleaved = match spec.sample_format {
        hound::SampleFormat::Float => r
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{}: {e}", path.display()))?,
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample.saturating_sub(1))) as f32;
            r.samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("{}: {e}", path.display()))?
        }
    };
    Ok(Wav {
        interleaved,
        sample_rate_hz: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
    })
}

pub fn read_f32_le(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() % 4 != 0 {
        return Err(format!("{}: length is not a multiple of 4", path.display()));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}
