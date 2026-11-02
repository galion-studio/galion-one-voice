// Galion Voice — Perpetual Open MIT License
//! Tiny std-only WAV / PCM helpers shared by `galion-render` (web surface) and
//! `galion-console`.
//!
//! Scope is deliberately small:
//! * [`parse_header`] — RIFF/WAVE header → [`WavInfo`] (walks chunks, accepts
//!   `WAVE_FORMAT_PCM` and `WAVE_FORMAT_EXTENSIBLE`, tolerates a streaming
//!   `data` size of `0` / `0xFFFF_FFFF` by clamping to the bytes present).
//! * [`decode_pcm16_mono`] — 16-bit PCM **mono** WAV → `(sample_rate, Vec<i16>)`.
//! * [`resample_linear`] / [`resample_16k_to_24k`] — linear-interpolation
//!   resampler; the voice path assumes **16 kHz in / 24 kHz out**
//!   ([`IN_RATE`] / [`OUT_RATE`]).
//! * [`encode_pcm16_mono`] — `Vec<i16>` → complete WAV bytes for a reply.
//! * [`decode_to_rate`] — parse + resample in one call (e.g. browser upload → 24 kHz).
//!
//! No allocation beyond the output buffers, no `unsafe`, no dependencies.

use std::fmt;

/// Sample rate the voice path assumes for microphone / uploaded input.
pub const IN_RATE: u32 = 16_000;
/// Sample rate the voice path produces for replies.
pub const OUT_RATE: u32 = 24_000;
/// Size of the canonical 44-byte header written by [`encode_pcm16_mono`].
pub const HEADER_LEN: usize = 44;

const FMT_PCM: u16 = 1;
const FMT_EXTENSIBLE: u16 = 0xFFFE;

/// Errors from WAV parsing / decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WavError {
    /// Fewer bytes than a RIFF/WAVE header needs, or a chunk runs past the end.
    Truncated,
    /// Missing `RIFF` / `WAVE` magic.
    NotWav,
    /// No `fmt ` chunk before `data`.
    MissingFmt,
    /// No `data` chunk.
    MissingData,
    /// Format tag other than integer PCM (e.g. float, A-law).
    UnsupportedFormat(u16),
    /// Valid WAV, but not 16-bit mono (`channels`, `bits_per_sample`).
    UnsupportedLayout { channels: u16, bits_per_sample: u16 },
    /// `sample_rate` of zero.
    BadSampleRate,
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WavError::Truncated => write!(f, "wav: truncated"),
            WavError::NotWav => write!(f, "wav: not a RIFF/WAVE file"),
            WavError::MissingFmt => write!(f, "wav: missing fmt chunk"),
            WavError::MissingData => write!(f, "wav: missing data chunk"),
            WavError::UnsupportedFormat(t) => write!(f, "wav: unsupported format tag {t:#06x} (need PCM)"),
            WavError::UnsupportedLayout { channels, bits_per_sample } => {
                write!(f, "wav: need 16-bit mono PCM, got {bits_per_sample}-bit x{channels}")
            }
            WavError::BadSampleRate => write!(f, "wav: sample rate is zero"),
        }
    }
}

impl std::error::Error for WavError {}

/// Parsed WAV header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    /// Format tag (1 = PCM; EXTENSIBLE is resolved to its sub-format).
    pub format: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// Byte offset of the first sample within the input.
    pub data_offset: usize,
    /// Number of sample bytes actually available (clamped to input length).
    pub data_len: usize,
}

impl WavInfo {
    /// Bytes per interleaved frame.
    pub fn block_align(&self) -> usize {
        self.channels as usize * (self.bits_per_sample as usize).div_ceil(8)
    }

    /// Number of whole frames in the data chunk.
    pub fn frames(&self) -> usize {
        match self.block_align() {
            0 => 0,
            b => self.data_len / b,
        }
    }

    /// Duration in milliseconds (floor).
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.frames() as u64 * 1000 / self.sample_rate as u64
    }

    /// True for 16-bit integer PCM mono — the only layout [`decode_pcm16_mono`] accepts.
    pub fn is_pcm16_mono(&self) -> bool {
        self.format == FMT_PCM && self.channels == 1 && self.bits_per_sample == 16
    }
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Parse a RIFF/WAVE header, walking chunks until `data`.
pub fn parse_header(b: &[u8]) -> Result<WavInfo, WavError> {
    if b.len() < 12 {
        return Err(WavError::Truncated);
    }
    if &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(WavError::NotWav);
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    loop {
        let (id, size) = match (b.get(pos..pos + 4), u32_at(b, pos + 4)) {
            (Some(id), Some(sz)) => (id, sz as usize),
            _ => return Err(if fmt.is_some() { WavError::MissingData } else { WavError::MissingFmt }),
        };
        let body = pos + 8;
        match id {
            b"fmt " => {
                if size < 16 || body + 16 > b.len() {
                    return Err(WavError::Truncated);
                }
                let mut tag = u16_at(b, body).unwrap();
                let channels = u16_at(b, body + 2).unwrap();
                let rate = u32_at(b, body + 4).unwrap();
                let bits = u16_at(b, body + 14).unwrap();
                if tag == FMT_EXTENSIBLE {
                    // cbSize(2) validBits(2) channelMask(4) subFormat GUID(16): first 2 bytes = tag.
                    tag = u16_at(b, body + 24).ok_or(WavError::Truncated)?;
                }
                fmt = Some((tag, channels, rate, bits));
            }
            b"data" => {
                let (format, channels, sample_rate, bits_per_sample) = fmt.ok_or(WavError::MissingFmt)?;
                if body > b.len() {
                    return Err(WavError::Truncated);
                }
                let avail = b.len() - body;
                // Streaming writers (browsers, ffmpeg pipes) leave 0 or u32::MAX here.
                let data_len = if size == 0 || size == u32::MAX as usize { avail } else { size.min(avail) };
                if sample_rate == 0 {
                    return Err(WavError::BadSampleRate);
                }
                return Ok(WavInfo { format, channels, sample_rate, bits_per_sample, data_offset: body, data_len });
            }
            _ => {}
        }
        // Chunks are word-aligned.
        pos = body.checked_add(size + (size & 1)).ok_or(WavError::Truncated)?;
    }
}

/// Decode a 16-bit PCM mono WAV into `(sample_rate, samples)`.
pub fn decode_pcm16_mono(b: &[u8]) -> Result<(u32, Vec<i16>), WavError> {
    let info = parse_header(b)?;
    if info.format != FMT_PCM {
        return Err(WavError::UnsupportedFormat(info.format));
    }
    if !info.is_pcm16_mono() {
        return Err(WavError::UnsupportedLayout { channels: info.channels, bits_per_sample: info.bits_per_sample });
    }
    let data = &b[info.data_offset..info.data_offset + info.data_len];
    Ok((info.sample_rate, pcm16_le_to_i16(data)))
}

/// Raw little-endian 16-bit PCM bytes → samples (a trailing odd byte is dropped).
pub fn pcm16_le_to_i16(data: &[u8]) -> Vec<i16> {
    data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
}

/// Samples → raw little-endian 16-bit PCM bytes (no header).
pub fn i16_to_pcm16_le(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Linear-interpolation resampler for mono i16. Output length is
/// `round(len * to / from)`. Identity when `from == to`; empty on zero rates.
pub fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64 * to as u64 + from as u64 / 2) / from as u64) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        // Source position in fixed point: i * from / to, integer + fractional part.
        let num = i as u64 * from as u64;
        let idx = (num / to as u64) as usize;
        let frac = (num % to as u64) as f64 / to as f64;
        let a = input[idx.min(last)] as f64;
        let b = input[(idx + 1).min(last)] as f64;
        let v = a + (b - a) * frac;
        out.push(v.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    out
}

/// Convenience: [`IN_RATE`] (16 kHz) → [`OUT_RATE`] (24 kHz).
pub fn resample_16k_to_24k(input: &[i16]) -> Vec<i16> {
    resample_linear(input, IN_RATE, OUT_RATE)
}

/// Parse a 16-bit mono WAV and resample it to `target_rate`.
pub fn decode_to_rate(b: &[u8], target_rate: u32) -> Result<Vec<i16>, WavError> {
    let (rate, s) = decode_pcm16_mono(b)?;
    Ok(resample_linear(&s, rate, target_rate))
}

/// Encode mono 16-bit PCM samples as a complete 44-byte-header WAV file.
pub fn encode_pcm16_mono(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(HEADER_LEN + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&FMT_PCM.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // channels
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Encode a reply at [`OUT_RATE`] (24 kHz).
pub fn encode_reply(samples: &[i16]) -> Vec<u8> {
    encode_pcm16_mono(OUT_RATE, samples)
}

/// `[-1.0, 1.0]` float → i16 (clamped).
pub fn f32_to_i16(x: &[f32]) -> Vec<i16> {
    x.iter().map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).collect()
}

/// i16 → `[-1.0, 1.0)` float.
pub fn i16_to_f32(x: &[i16]) -> Vec<f32> {
    x.iter().map(|&v| v as f32 / 32768.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<i16> {
        (0..n).map(|i| (i as i32 * 100 - 5000) as i16).collect()
    }

    #[test]
    fn encode_then_parse_roundtrip() {
        let s = ramp(160);
        let w = encode_pcm16_mono(16_000, &s);
        assert_eq!(w.len(), HEADER_LEN + 320);
        let info = parse_header(&w).unwrap();
        assert!(info.is_pcm16_mono());
        assert_eq!(info.sample_rate, 16_000);
        assert_eq!(info.data_offset, 44);
        assert_eq!(info.frames(), 160);
        assert_eq!(info.duration_ms(), 10);
        assert_eq!(decode_pcm16_mono(&w).unwrap(), (16_000, s));
    }

    #[test]
    fn header_fields_are_canonical() {
        let w = encode_reply(&[1, -1]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32_at(&w, 4), Some(36 + 4));
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(&w, 24), Some(OUT_RATE));
        assert_eq!(u32_at(&w, 28), Some(OUT_RATE * 2));
        assert_eq!(u16_at(&w, 32), Some(2));
        assert_eq!(&w[36..40], b"data");
        assert_eq!(u32_at(&w, 40), Some(4));
    }

    #[test]
    fn skips_unknown_and_odd_chunks() {
        let s = ramp(8);
        let w = encode_pcm16_mono(16_000, &s);
        let mut x = w[..12].to_vec();
        x.extend_from_slice(b"LIST");
        x.extend_from_slice(&3u32.to_le_bytes());
        x.extend_from_slice(&[9, 9, 9, 0]); // 3 bytes + pad
        x.extend_from_slice(&w[12..]);
        assert_eq!(decode_pcm16_mono(&x).unwrap().1, s);
    }

    #[test]
    fn streaming_data_size_is_clamped() {
        let s = ramp(10);
        let mut w = encode_pcm16_mono(16_000, &s);
        w[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode_pcm16_mono(&w).unwrap().1, s);
        w[40..44].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode_pcm16_mono(&w).unwrap().1, s);
        // Declared larger than present: clamp, drop odd trailing byte.
        w[40..44].copy_from_slice(&1000u32.to_le_bytes());
        w.push(7);
        assert_eq!(decode_pcm16_mono(&w).unwrap().1, s);
    }

    #[test]
    fn extensible_pcm_is_accepted() {
        let s = ramp(4);
        let mut x = Vec::new();
        x.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        x.extend_from_slice(&40u32.to_le_bytes());
        x.extend_from_slice(&FMT_EXTENSIBLE.to_le_bytes());
        x.extend_from_slice(&1u16.to_le_bytes());
        x.extend_from_slice(&16_000u32.to_le_bytes());
        x.extend_from_slice(&32_000u32.to_le_bytes());
        x.extend_from_slice(&2u16.to_le_bytes());
        x.extend_from_slice(&16u16.to_le_bytes());
        x.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        x.extend_from_slice(&16u16.to_le_bytes()); // valid bits
        x.extend_from_slice(&4u32.to_le_bytes()); // mask
        x.extend_from_slice(&1u16.to_le_bytes()); // subformat PCM
        x.extend_from_slice(&[0u8; 14]);
        x.extend_from_slice(b"data");
        x.extend_from_slice(&8u32.to_le_bytes());
        x.extend_from_slice(&i16_to_pcm16_le(&s));
        assert_eq!(decode_pcm16_mono(&x).unwrap(), (16_000, s));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(parse_header(b"RIFF"), Err(WavError::Truncated));
        assert_eq!(parse_header(b"RIFX\0\0\0\0WAVEjunk"), Err(WavError::NotWav));
        assert_eq!(parse_header(b"RIFF\0\0\0\0WAVE"), Err(WavError::MissingFmt));
        let w = encode_pcm16_mono(16_000, &[0; 4]);
        assert_eq!(parse_header(&w[..36]), Err(WavError::MissingData));
        // Stereo.
        let mut st = w.clone();
        st[22..24].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            decode_pcm16_mono(&st),
            Err(WavError::UnsupportedLayout { channels: 2, bits_per_sample: 16 })
        );
        // Float.
        let mut fl = w.clone();
        fl[20..22].copy_from_slice(&3u16.to_le_bytes());
        assert_eq!(decode_pcm16_mono(&fl), Err(WavError::UnsupportedFormat(3)));
        // Zero rate.
        let mut z = w.clone();
        z[24..28].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse_header(&z), Err(WavError::BadSampleRate));
        assert!(!WavError::Truncated.to_string().is_empty());
    }

    #[test]
    fn resample_16k_to_24k_lengths_and_values() {
        let s = ramp(160); // 10 ms
        let r = resample_16k_to_24k(&s);
        assert_eq!(r.len(), 240);
        // Every 3rd output sample lands exactly on an input sample (ratio 2:3).
        for k in 0..80 {
            assert_eq!(r[3 * k], s[2 * k]);
        }
        // Linear ramp stays linear (midpoints interpolate).
        assert_eq!(r[1], ((s[0] as f64 + (s[1] - s[0]) as f64 * (2.0 / 3.0)).round()) as i16);
        assert_eq!(resample_linear(&s, 24_000, 16_000).len(), 107);
    }

    #[test]
    fn resample_edge_cases() {
        assert_eq!(resample_linear(&[1, 2, 3], 16_000, 16_000), vec![1, 2, 3]);
        assert!(resample_linear(&[], 16_000, 24_000).is_empty());
        assert!(resample_linear(&[1], 0, 24_000).is_empty());
        assert_eq!(resample_linear(&[i16::MAX, i16::MIN], 16_000, 48_000).len(), 6);
        assert_eq!(resample_linear(&[5], 16_000, 24_000), vec![5, 5]);
    }

    #[test]
    fn decode_to_rate_end_to_end() {
        let s = ramp(1600); // 100 ms @ 16 kHz
        let w = encode_pcm16_mono(IN_RATE, &s);
        let out = decode_to_rate(&w, OUT_RATE).unwrap();
        assert_eq!(out.len(), 2400);
        let reply = encode_reply(&out);
        let info = parse_header(&reply).unwrap();
        assert_eq!((info.sample_rate, info.frames(), info.duration_ms()), (OUT_RATE, 2400, 100));
    }

    #[test]
    fn float_conversions_clamp() {
        assert_eq!(f32_to_i16(&[0.0, 1.0, -1.0, 2.0, -2.0]), vec![0, 32767, -32767, 32767, -32767]);
        let f = i16_to_f32(&[0, -32768, 16384]);
        assert_eq!(f, vec![0.0, -1.0, 0.5]);
        assert_eq!(pcm16_le_to_i16(&i16_to_pcm16_le(&[1, -2, 300])), vec![1, -2, 300]);
    }
}
