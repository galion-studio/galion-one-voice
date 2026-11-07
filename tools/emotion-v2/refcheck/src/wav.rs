// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! RIFF/WAVE reader and writer for the capture checker.
//!
//! Integer PCM (8/16/24/32) and IEEE float 32. Packed 24-bit is three bytes per
//! sample. Unknown chunks are skipped. A truncated data chunk is an error.

use std::fs::OpenOptions;
use std::path::Path;

const PCM_GUID_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    PcmInteger,
    PcmFloat,
}

impl SampleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SampleKind::PcmInteger => "pcm_integer",
            SampleKind::PcmFloat => "pcm_float",
        }
    }
}

/// Decoded audio. `samples` is the downmix used for level and spectrum.
/// `clipped_samples` counts full-scale codes on every channel before the mix.
#[derive(Debug, Clone)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub kind: SampleKind,
    pub samples: Vec<f32>,
    pub clipped_samples: u64,
}

#[derive(Debug, Clone)]
pub struct EncodeSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub kind: SampleKind,
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Read a WAV without opening it for write.
pub fn read_wav(path: &Path) -> Result<Pcm, String> {
    let f = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .map_err(|e| format!("wav: cannot read {}: {e}", path.display()))?;
    let meta = f
        .metadata()
        .map_err(|e| format!("wav: {}: {e}", path.display()))?;
    if meta.len() as usize > MAX_BYTES {
        return Err(format!(
            "wav: {} is over 64 MB; a capture take is a few seconds",
            path.display()
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|e| format!("wav: cannot read {}: {e}", path.display()))?;
    parse_wav(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse_wav(b: &[u8]) -> Result<Pcm, String> {
    if b.len() > MAX_BYTES {
        return Err("wav: file is over 64 MB".into());
    }
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("wav: not a RIFF/WAVE file".into());
    }
    let mut i = 12usize;
    let mut fmt: Option<Fmt> = None;
    let mut data: Option<&[u8]> = None;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32le(b, i + 4) as usize;
        let body = i + 8;
        let end = body.checked_add(len).ok_or("wav: chunk size overflow")?;
        if end > b.len() {
            return Err("wav: truncated chunk".into());
        }
        match id {
            b"fmt " => fmt = Some(parse_fmt(&b[body..end])?),
            b"data" => data = Some(&b[body..end]),
            _ => {}
        }
        let next = end + (len & 1);
        if next > b.len() {
            break;
        }
        i = next;
        if fmt.is_some() && data.is_some() && i + 8 > b.len() {
            break;
        }
    }
    let fmt = fmt.ok_or("wav: missing fmt chunk")?;
    let data = data.ok_or("wav: missing data chunk")?;
    decode(&fmt, data)
}

struct Fmt {
    kind: SampleKind,
    channels: u16,
    rate: u32,
    bits: u16,
}

fn parse_fmt(b: &[u8]) -> Result<Fmt, String> {
    if b.len() < 16 {
        return Err("wav: fmt chunk too short".into());
    }
    let tag = u16le(b, 0);
    let channels = u16le(b, 2);
    let rate = u32le(b, 4);
    let block_align = u16le(b, 12);
    let bits = u16le(b, 14);
    let kind = match tag {
        1 => SampleKind::PcmInteger,
        3 => SampleKind::PcmFloat,
        0xFFFE => {
            if b.len() < 40 {
                return Err("wav: extensible fmt is short".into());
            }
            let sub = u16le(b, 24);
            if b[26..40] != PCM_GUID_TAIL {
                return Err("wav: extensible subformat is not PCM or IEEE float".into());
            }
            match sub {
                1 => SampleKind::PcmInteger,
                3 => SampleKind::PcmFloat,
                _ => {
                    return Err(format!(
                        "wav: extensible subformat {sub} is not PCM or IEEE float"
                    ))
                }
            }
        }
        _ => {
            return Err(format!(
                "wav: format tag 0x{tag:04x} is not integer PCM or IEEE float"
            ))
        }
    };
    if channels == 0 || channels > 8 {
        return Err(format!("wav: {channels} channels (1..=8)"));
    }
    if !(8_000..=192_000).contains(&rate) {
        return Err(format!(
            "wav: sample rate {rate} Hz is outside 8000..=192000"
        ));
    }
    match kind {
        SampleKind::PcmInteger => {
            if ![8, 16, 24, 32].contains(&bits) {
                return Err(format!("wav: {bits}-bit integer PCM is not 8/16/24/32"));
            }
        }
        SampleKind::PcmFloat => {
            if bits != 32 {
                return Err(format!("wav: {bits}-bit float is not 32-bit IEEE float"));
            }
        }
    }
    let expect = channels as u32 * bits as u32 / 8;
    if block_align as u32 != expect {
        return Err(format!("wav: block_align {block_align} != {expect}"));
    }
    Ok(Fmt {
        kind,
        channels,
        rate,
        bits,
    })
}

fn decode(fmt: &Fmt, data: &[u8]) -> Result<Pcm, String> {
    let bps = fmt.bits as usize / 8;
    let frame = bps * fmt.channels as usize;
    if frame == 0 || data.len() % frame != 0 {
        return Err("wav: data length is not a whole number of frames".into());
    }
    let n = data.len() / frame;
    if n == 0 {
        return Err("wav: empty audio".into());
    }
    let mut samples = Vec::with_capacity(n);
    let mut clipped = 0u64;
    for k in 0..n {
        let mut acc = 0.0f32;
        for c in 0..fmt.channels as usize {
            let o = k * frame + c * bps;
            let (s, rail) = decode_one(&fmt.kind, fmt.bits, &data[o..o + bps])?;
            if rail {
                clipped += 1;
            }
            acc += s;
        }
        samples.push(acc / fmt.channels as f32);
    }
    if samples.iter().any(|s| !s.is_finite()) {
        return Err("wav: non-finite sample".into());
    }
    Ok(Pcm {
        sample_rate: fmt.rate,
        channels: fmt.channels,
        bits_per_sample: fmt.bits,
        kind: fmt.kind,
        samples,
        clipped_samples: clipped,
    })
}

fn decode_one(kind: &SampleKind, bits: u16, b: &[u8]) -> Result<(f32, bool), String> {
    match kind {
        SampleKind::PcmFloat => {
            let s = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            Ok((s, s.is_finite() && s.abs() >= 1.0))
        }
        SampleKind::PcmInteger => match bits {
            8 => {
                let u = b[0];
                let rail = u == 0 || u == 255;
                Ok(((u as f32 - 128.0) / 128.0, rail))
            }
            16 => {
                let v = i16::from_le_bytes([b[0], b[1]]);
                let rail = v == i16::MAX || v == i16::MIN;
                Ok((v as f32 / 32768.0, rail))
            }
            24 => {
                let v = (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as i32;
                let rail = v == 8_388_607 || v == -8_388_608;
                Ok((v as f32 / 8_388_608.0, rail))
            }
            32 => {
                let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let rail = v == i32::MAX || v == i32::MIN;
                Ok((v as f32 / 2_147_483_648.0, rail))
            }
            _ => Err(format!("wav: {bits}-bit integer is unsupported")),
        },
    }
}

pub fn encode(spec: &EncodeSpec, interleaved: &[f32]) -> Result<Vec<u8>, String> {
    if spec.channels == 0 || spec.channels > 8 {
        return Err("wav: encode channels must be 1..=8".into());
    }
    if interleaved.is_empty() || interleaved.len() % spec.channels as usize != 0 {
        return Err("wav: encode length is not a whole number of frames".into());
    }
    if interleaved.iter().any(|s| !s.is_finite()) {
        return Err("wav: encode sample is non-finite".into());
    }
    let bps = match spec.kind {
        SampleKind::PcmFloat => {
            if spec.bits_per_sample != 32 {
                return Err("wav: float encode is 32-bit".into());
            }
            4
        }
        SampleKind::PcmInteger => match spec.bits_per_sample {
            16 => 2,
            24 => 3,
            32 => 4,
            _ => return Err("wav: integer encode is 16, 24, or 32 bit".into()),
        },
    };
    let data_len = interleaved.len() * bps;
    let mut v = Vec::with_capacity(44 + data_len);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    let tag: u16 = match spec.kind {
        SampleKind::PcmInteger => 1,
        SampleKind::PcmFloat => 3,
    };
    v.extend_from_slice(&tag.to_le_bytes());
    v.extend_from_slice(&spec.channels.to_le_bytes());
    v.extend_from_slice(&spec.sample_rate.to_le_bytes());
    let byte_rate = spec.sample_rate * spec.channels as u32 * bps as u32;
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&(spec.channels * bps as u16).to_le_bytes());
    v.extend_from_slice(&spec.bits_per_sample.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_len as u32).to_le_bytes());
    for &s in interleaved {
        match spec.kind {
            SampleKind::PcmFloat => v.extend_from_slice(&s.to_le_bytes()),
            SampleKind::PcmInteger => match spec.bits_per_sample {
                16 => {
                    let q = (s.clamp(-1.0, 1.0) * 32768.0)
                        .round()
                        .clamp(-32768.0, 32767.0) as i16;
                    v.extend_from_slice(&q.to_le_bytes());
                }
                24 => {
                    let q = (s.clamp(-1.0, 1.0) * 8_388_608.0)
                        .round()
                        .clamp(-8_388_608.0, 8_388_607.0) as i32;
                    let b = q.to_le_bytes();
                    v.extend_from_slice(&b[0..3]);
                }
                _ => {
                    let q = (s.clamp(-1.0, 1.0) * 2_147_483_648.0)
                        .round()
                        .clamp(-2_147_483_648.0, 2_147_483_647.0)
                        as i32;
                    v.extend_from_slice(&q.to_le_bytes());
                }
            },
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_24_bit_roundtrip_half_scale() {
        let spec = EncodeSpec {
            sample_rate: 48_000,
            channels: 1,
            bits_per_sample: 24,
            kind: SampleKind::PcmInteger,
        };
        let bytes = encode(&spec, &[0.5, -0.5, 0.0, 1.0, -1.0]).unwrap();
        let pcm = parse_wav(&bytes).unwrap();
        assert_eq!(
            (pcm.sample_rate, pcm.channels, pcm.bits_per_sample),
            (48_000, 1, 24)
        );
        assert_eq!(pcm.kind, SampleKind::PcmInteger);
        assert!((pcm.samples[0] - 0.5).abs() < 1e-6);
        assert!((pcm.samples[1] + 0.5).abs() < 1e-6);
        assert_eq!(pcm.samples[2], 0.0);
        assert_eq!(pcm.clipped_samples, 2, "both rails count");
    }

    #[test]
    fn skips_a_list_chunk_and_rejects_truncation() {
        let spec = EncodeSpec {
            sample_rate: 16_000,
            channels: 1,
            bits_per_sample: 16,
            kind: SampleKind::PcmInteger,
        };
        let mut bytes = encode(&spec, &[0.25]).unwrap();
        // Insert a LIST chunk before data by rebuilding is unnecessary: parser skips unknown
        // chunks wherever they sit. Prepend is illegal (RIFF header). Splice after fmt.
        let list = b"LIST\x04\x00\x00\x00abcd";
        bytes.splice(36..36, list.iter().copied());
        let riff = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff.to_le_bytes());
        let pcm = parse_wav(&bytes).unwrap();
        assert!((pcm.samples[0] - 0.25).abs() < 1e-4);
        bytes.pop();
        assert!(parse_wav(&bytes).is_err());
    }
}
