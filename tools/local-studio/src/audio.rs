// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! WAV, fades, resampling, and hashing. Tones written here are generated.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn read_wav_mono(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    if channels == 0 || spec.sample_rate == 0 {
        return Err(format!("{}: wav header is empty", path.display()));
    }
    let frames = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?,
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample.saturating_sub(1).max(1) - 0)) as f32;
            let scale = if spec.bits_per_sample <= 8 {
                128.0
            } else if spec.bits_per_sample <= 16 {
                32768.0
            } else {
                scale.max(1.0)
            };
            if spec.bits_per_sample <= 16 {
                reader
                    .samples::<i16>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| err.to_string())?
            } else {
                reader
                    .samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| err.to_string())?
            }
        }
    };
    Ok((fold_mono(&frames, channels), spec.sample_rate))
}

fn fold_mono(frames: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return frames.to_vec();
    }
    let n = frames.len() / channels;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut acc = 0.0f32;
        for c in 0..channels {
            acc += frames[i * channels + c];
        }
        out.push(acc / channels as f32);
    }
    out
}

pub fn write_wav_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("{}: {err}", parent.display()))?;
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|err| format!("{}: {err}", path.display()))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        writer.write_sample(value).map_err(|err| err.to_string())?;
    }
    writer.finalize().map_err(|err| err.to_string())
}

pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 || from == to || input.is_empty() {
        return input.to_vec();
    }
    let n = ((input.len() as u64) * u64::from(to) / u64::from(from)).max(1) as usize;
    let last = input.len() - 1;
    let scale = f64::from(from) / f64::from(to);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = i as f64 * scale;
        let i0 = (pos.floor() as usize).min(last);
        let i1 = (i0 + 1).min(last);
        let frac = (pos - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}

pub fn fade_edges(samples: &mut [f32], sample_rate: u32, fade_ms: u32) {
    if samples.len() < 4 || sample_rate == 0 || fade_ms == 0 {
        return;
    }
    let n = ((u64::from(sample_rate) * u64::from(fade_ms)) / 1000).max(1) as usize;
    let n = n.min(samples.len() / 2);
    for i in 0..n {
        let gain = i as f32 / n as f32;
        samples[i] *= gain;
        let j = samples.len() - 1 - i;
        samples[j] *= gain;
    }
}

pub fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let acc = samples
        .iter()
        .map(|sample| {
            let v = f64::from(*sample);
            v * v
        })
        .sum::<f64>();
    (acc / samples.len() as f64).sqrt()
}

pub fn rms_ranges(samples: &[f32], ranges: &[(usize, usize)]) -> f64 {
    let mut acc = 0.0f64;
    let mut n = 0usize;
    for &(start, end) in ranges {
        let end = end.min(samples.len());
        let start = start.min(end);
        for sample in &samples[start..end] {
            let v = f64::from(*sample);
            acc += v * v;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        (acc / n as f64).sqrt()
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub struct StitchPiece<'a> {
    pub samples: &'a [f32],
    pub gap_after: usize,
}

pub struct Stitched {
    pub samples: Vec<f32>,
    pub speech: Vec<(usize, usize)>,
}

pub fn stitch(lead: usize, pieces: &[StitchPiece<'_>], tail: usize) -> Stitched {
    let mut total = lead + tail;
    for piece in pieces {
        total += piece.samples.len() + piece.gap_after;
    }
    let mut samples = vec![0.0f32; total];
    let mut speech = Vec::with_capacity(pieces.len());
    let mut cursor = lead;
    for piece in pieces {
        let end = cursor + piece.samples.len();
        samples[cursor..end].copy_from_slice(piece.samples);
        speech.push((cursor, end));
        cursor = end + piece.gap_after;
    }
    Stitched { samples, speech }
}

pub fn mix_under(voice: &[f32], bed: &[f32], speech: &[(usize, usize)], under_db: f64) -> Vec<f32> {
    let voice_rms = rms_ranges(voice, speech);
    let bed_rms = rms_ranges(bed, speech);
    if voice_rms < 1e-8 || bed_rms < 1e-10 {
        return voice.to_vec();
    }
    let gain = ((voice_rms / bed_rms) * 10f64.powf(-under_db / 20.0)) as f32;
    let mut mixed = Vec::with_capacity(voice.len());
    for (i, sample) in voice.iter().enumerate() {
        let bed_sample = bed.get(i).copied().unwrap_or(0.0) * gain;
        mixed.push(sample + bed_sample);
    }
    mixed
}

pub fn ms_samples(sample_rate: u32, ms: u32) -> usize {
    ((u64::from(sample_rate) * u64::from(ms)) / 1000) as usize
}

pub fn sine(n: usize, sample_rate: u32, freq_hz: f64, amplitude: f32) -> Vec<f32> {
    let sr = f64::from(sample_rate);
    (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            (amplitude as f64 * (2.0 * std::f64::consts::PI * freq_hz * t).sin()) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_keeps_a_tone() {
        let dir = std::env::temp_dir().join(format!("galion-studio-wav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let tone = sine(24_000, 24_000, 432.0, 0.2);
        write_wav_mono(&path, &tone, 24_000).unwrap();
        let (back, sr) = read_wav_mono(&path).unwrap();
        assert_eq!(sr, 24_000);
        assert_eq!(back.len(), tone.len());
        let err = tone
            .iter()
            .zip(&back)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1.0 / 32768.0 + 1e-4, "{err}");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(sha256_file(&path).unwrap(), sha256_bytes(&bytes));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mix_sits_eighteen_db_under_the_voice() {
        let n = 24_000;
        let voice = sine(n, 24_000, 432.0, 0.2);
        let bed = sine(n, 24_000, 216.0, 0.2);
        let mixed = mix_under(&voice, &bed, &[(0, n)], 18.0);
        let mut bed_only = vec![0.0f32; n];
        for i in 0..n {
            bed_only[i] = mixed[i] - voice[i];
        }
        let db = 20.0 * (rms(&voice) / rms(&bed_only)).log10();
        assert!((db - 18.0).abs() < 0.05, "{db}");
    }

    #[test]
    fn stitch_places_speech_after_the_lead() {
        let a = vec![0.2f32; 10];
        let b = vec![0.3f32; 4];
        let stitched = stitch(
            5,
            &[
                StitchPiece {
                    samples: &a,
                    gap_after: 3,
                },
                StitchPiece {
                    samples: &b,
                    gap_after: 2,
                },
            ],
            7,
        );
        assert_eq!(stitched.speech, vec![(5, 15), (18, 22)]);
        assert_eq!(stitched.samples.len(), 5 + 10 + 3 + 4 + 2 + 7);
        assert_eq!(stitched.samples[5], 0.2);
        assert_eq!(stitched.samples[18], 0.3);
        assert_eq!(stitched.samples[15], 0.0);
    }
}
