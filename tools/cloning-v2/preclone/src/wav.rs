// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! WAV in, through hound. The stage reads a reference. It does not write one.

use std::path::Path;

pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("wav: {}", e))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err("wav: empty format".into());
    }
    let ch = spec.channels as usize;
    let interleaved = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("wav: {e}"))?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            if bits == 0 || bits > 32 {
                return Err(format!("wav: {bits}-bit samples are not read"));
            }
            let scale = (1i64 << (bits - 1)) as f32;
            if bits <= 16 {
                reader
                    .samples::<i16>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("wav: {e}"))?
            } else {
                reader
                    .samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("wav: {e}"))?
            }
        }
    };
    let mono = if ch == 1 {
        interleaved
    } else {
        interleaved
            .chunks(ch)
            .filter(|f| f.len() == ch)
            .map(|f| f.iter().sum::<f32>() / ch as f32)
            .collect()
    };
    Ok((mono, spec.sample_rate))
}
