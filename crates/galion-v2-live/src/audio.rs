// Galion Voice — Perpetual Open MIT License
//! WAV in/out (PCM s16 mono/stereo) and linear resampling. No codec deps.
use std::io::{Read, Write};

pub struct Wav { pub rate: u32, pub channels: u16, pub samples: Vec<i16> }

pub fn read_wav(path: &str) -> Result<Wav, String> {
    let mut b = Vec::new();
    std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?.read_to_end(&mut b).map_err(|e| e.to_string())?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" { return Err(format!("{path}: not RIFF/WAVE")); }
    let (mut i, mut rate, mut ch, mut bits) = (12usize, 0u32, 0u16, 0u16);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let sz = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = &b[i + 8..(i + 8 + sz).min(b.len())];
        if id == b"fmt " {
            ch = u16::from_le_bytes(body[2..4].try_into().unwrap());
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            if bits != 16 { return Err(format!("{path}: {bits}-bit, want 16")); }
            let s = body.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
            return Ok(Wav { rate, channels: ch, samples: s });
        }
        i += 8 + sz + (sz & 1);
    }
    Err(format!("{path}: no data chunk"))
}

pub fn to_mono(w: &Wav) -> Vec<i16> {
    if w.channels <= 1 { return w.samples.clone(); }
    let c = w.channels as usize;
    w.samples.chunks_exact(c).map(|f| (f.iter().map(|&s| s as i32).sum::<i32>() / c as i32) as i16).collect()
}

pub fn write_wav(path: &str, rate: u32, channels: u16, s: &[i16]) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data = (s.len() * 2) as u32;
    f.write_all(b"RIFF")?; f.write_all(&(36 + data).to_le_bytes())?; f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?; f.write_all(&1u16.to_le_bytes())?; f.write_all(&channels.to_le_bytes())?;
    f.write_all(&rate.to_le_bytes())?; f.write_all(&(rate * channels as u32 * 2).to_le_bytes())?;
    f.write_all(&(channels * 2).to_le_bytes())?; f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?; f.write_all(&data.to_le_bytes())?;
    for v in s { f.write_all(&v.to_le_bytes())?; }
    Ok(())
}

/// Linear resample (adequate for ASR feed / mixing; not a mastering SRC).
pub fn resample(x: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == to || x.is_empty() { return x.to_vec(); }
    let n = (x.len() as u64 * to as u64 / from as u64) as usize;
    let step = from as f64 / to as f64;
    (0..n).map(|i| {
        let p = i as f64 * step; let j = p as usize; let fr = p - j as f64;
        let a = x[j.min(x.len() - 1)] as f64; let b = x[(j + 1).min(x.len() - 1)] as f64;
        (a + (b - a) * fr).round() as i16
    }).collect()
}

pub fn bytes_to_i16(b: &[u8]) -> Vec<i16> { b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect() }
pub fn rms_db(x: &[i16]) -> f64 {
    if x.is_empty() { return -120.0; }
    let e = x.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum::<f64>() / x.len() as f64;
    10.0 * e.max(1e-12).log10()
}
