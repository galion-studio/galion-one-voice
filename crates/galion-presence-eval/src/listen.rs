// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Read a captured reply WAV. The virtual user scores what is in the file,
//! not a timestamp the synthesizer printed beside it.

/// Mono PCM16 RIFF/WAVE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavPcm {
    pub rate: u32,
    pub samples: Vec<i16>,
}

pub fn parse_pcm16(b: &[u8]) -> Result<WavPcm, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not RIFF/WAVE".into());
    }
    let (mut fmt, mut data) = (None, None);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let end = (i + 8 + n).min(b.len());
        let body = &b[i + 8..end];
        if id == b"fmt " {
            fmt = Some(body.to_vec());
        } else if id == b"data" {
            data = Some(body.to_vec());
        }
        i += 8 + n + (n & 1);
    }
    let fmt = fmt.ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    if fmt.len() < 16 {
        return Err("short fmt chunk".into());
    }
    let u16le = |o: usize| u16::from_le_bytes([fmt[o], fmt[o + 1]]);
    let tag = u16le(0);
    let channels = u16le(2).max(1) as usize;
    let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16le(14);
    if tag != 1 || bits != 16 {
        return Err("only PCM16 is accepted".into());
    }
    if rate == 0 {
        return Err("sample rate 0".into());
    }
    if channels != 1 {
        return Err("reply WAV must be mono".into());
    }
    let samples = data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok(WavPcm { rate, samples })
}

pub fn peak(samples: &[i16]) -> u32 {
    samples
        .iter()
        .map(|s| s.unsigned_abs() as u32)
        .max()
        .unwrap_or(0)
}

/// Milliseconds from the start of the buffer to the first 20 ms window with RMS above 0.01.
pub fn first_voiced_ms(samples: &[i16], rate: u32) -> Option<u32> {
    if rate == 0 || samples.is_empty() {
        return None;
    }
    let win = (rate as usize / 50).max(1);
    for (i, chunk) in samples.chunks(win).enumerate() {
        let energy = chunk
            .iter()
            .map(|&s| {
                let x = s as f32 / 32768.0;
                x * x
            })
            .sum::<f32>()
            / chunk.len() as f32;
        if energy.sqrt() > 0.01 {
            return Some(((i as u64 * win as u64 * 1000) / rate as u64) as u32);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_cascade::en_lane::wav_bytes;

    #[test]
    fn silence_has_no_voice_and_a_tone_does() {
        let silent = wav_bytes(&[0; 16_000], 16_000);
        let pcm = parse_pcm16(&silent).unwrap();
        assert_eq!(pcm.rate, 16_000);
        assert_eq!(peak(&pcm.samples), 0);
        assert_eq!(first_voiced_ms(&pcm.samples, pcm.rate), None);
        let tone: Vec<i16> = (0..16_000)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                (0.2 * (t * 220.0 * std::f32::consts::TAU).sin() * 32767.0) as i16
            })
            .collect();
        let lead = std::iter::repeat(0)
            .take(3_200)
            .chain(tone)
            .collect::<Vec<_>>();
        let heard = first_voiced_ms(&lead, 16_000).unwrap();
        assert!((180..250).contains(&heard), "{heard}");
        assert!(peak(&lead) > 1500);
    }
}
