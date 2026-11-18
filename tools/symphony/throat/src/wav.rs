// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! 16-bit PCM for the optional reference. The file is peak-scaled to 0.5.
//! That scale is not Ee, and the file is not a calibration of loudness.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let mut peak = 0.0f32;
    for s in samples {
        peak = peak.max(s.abs());
    }
    let gain = if peak > 0.0 { 0.5 / peak } else { 1.0 };
    let mut body = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let y = (s * gain).clamp(-1.0, 1.0);
        let q = (y * 32767.0).round() as i16;
        body.extend_from_slice(&q.to_le_bytes());
    }
    let mut w = BufWriter::new(File::create(path).map_err(|e| format!("create wav: {e}"))?);
    let data_len = body.len() as u32;
    let riff_len = 36 + data_len;
    w.write_all(b"RIFF").map_err(io)?;
    w.write_all(&riff_len.to_le_bytes()).map_err(io)?;
    w.write_all(b"WAVE").map_err(io)?;
    w.write_all(b"fmt ").map_err(io)?;
    w.write_all(&16u32.to_le_bytes()).map_err(io)?;
    w.write_all(&1u16.to_le_bytes()).map_err(io)?;
    w.write_all(&1u16.to_le_bytes()).map_err(io)?;
    w.write_all(&sample_rate.to_le_bytes()).map_err(io)?;
    let byte_rate = sample_rate * 2;
    w.write_all(&byte_rate.to_le_bytes()).map_err(io)?;
    w.write_all(&2u16.to_le_bytes()).map_err(io)?;
    w.write_all(&16u16.to_le_bytes()).map_err(io)?;
    w.write_all(b"data").map_err(io)?;
    w.write_all(&data_len.to_le_bytes()).map_err(io)?;
    w.write_all(&body).map_err(io)?;
    w.flush().map_err(io)?;
    Ok(())
}

fn io(e: std::io::Error) -> String {
    format!("write wav: {e}")
}
