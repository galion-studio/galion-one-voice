// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Clip ABI v1 (`.gclip`).
//!
//! The interchange is a WAV file. The symphony object is this container.
//! The payload is the WAV bytes, unchanged. Nothing in this module downmixes,
//! resamples, denoises, or clears a channel. `texture_strip` is stored and
//! must be 0. There is no strip call.
//!
//! This is not a worker request field. The clone child still refuses a
//! reference on stdin. Rust loads the clip with [`open`].
//!
//! ```text
//!  0  8   magic GALIONCL
//!  8  2   version = 1
//! 10  2   flags = 0
//! 12  2   persona (1 samantha, 2 jarvis)
//! 14  2   texture policy (1 keep, 2 voice)
//! 16  2   texture_strip (0 only)
//! 18  2   texture bits (bit0 clicks, bit1 alarms, bit2 ambience)
//! 20  2   channels
//! 22  2   bits per sample
//! 24  4   sample rate
//! 28  2   audio format (1 PCM)
//! 30  2   reserved 0
//! 32  8   payload length
//! 40  8   duration milliseconds
//! 48 32   SHA-256 of the payload
//! 80 32   reserved 0
//! 112     payload: the original RIFF/WAVE
//! ```

use crate::persona::{PersonaId, RefExpect, TexturePolicy};
use crate::sha::{self, Sha256};
use crate::wav;
use crate::{Error, Result};
use std::path::Path;

pub const MAGIC: [u8; 8] = *b"GALIONCL";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 112;
/// Owner references are a few megabytes. A longer file is the wrong object.
pub const MAX_WAV: u64 = 32 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipHeader {
    pub persona: PersonaId,
    pub texture: TexturePolicy,
    pub texture_strip: u16,
    pub texture_bits: u16,
    pub channels: u16,
    pub bits: u16,
    pub sample_rate: u32,
    pub audio_format: u16,
    pub payload_len: u64,
    pub duration_ms: u64,
    pub payload_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub header: ClipHeader,
    payload: Vec<u8>,
}

impl Clip {
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn interleaved_i16(&self) -> Result<Vec<i16>> {
        wav::interleaved_i16(&self.payload)
    }
}

pub struct WrapOpts<'a> {
    pub persona: PersonaId,
    pub wav: &'a Path,
    pub out: &'a Path,
    /// When set, the WAV sha256 and PCM shape must match the published reference.
    pub check_pin: bool,
}

pub struct WrapBytes<'a> {
    pub persona: PersonaId,
    pub wav: &'a [u8],
    pub out: &'a Path,
    /// When set, the WAV sha256 and PCM shape must match the published reference.
    pub check_pin: bool,
}

pub fn wrap(opts: WrapOpts<'_>) -> Result<ClipHeader> {
    let meta =
        std::fs::metadata(opts.wav).map_err(|e| Error::io(&opts.wav.display().to_string(), e))?;
    if meta.len() == 0 || meta.len() > MAX_WAV {
        return Err(Error::Refused(format!(
            "{} is {} bytes; clip payload must be 1..={MAX_WAV}",
            opts.wav.display(),
            meta.len()
        )));
    }
    let bytes =
        std::fs::read(opts.wav).map_err(|e| Error::io(&opts.wav.display().to_string(), e))?;
    wrap_bytes(WrapBytes {
        persona: opts.persona,
        wav: &bytes,
        out: opts.out,
        check_pin: opts.check_pin,
    })
}

/// Wrap WAV bytes the caller already holds. This does not open a wav path.
pub fn wrap_bytes(opts: WrapBytes<'_>) -> Result<ClipHeader> {
    let len = opts.wav.len() as u64;
    if len == 0 || len > MAX_WAV {
        return Err(Error::Refused(format!(
            "clip payload is {len} bytes; clip payload must be 1..={MAX_WAV}"
        )));
    }
    let digest = sha::sha256(opts.wav);
    if opts.check_pin {
        let expect = opts.persona.ref_expect();
        check_published(&expect, &digest, opts.wav)?;
    }
    let header = header_for(opts.persona, opts.wav, digest)?;
    write_file(opts.out, &header, opts.wav)?;
    Ok(header)
}

fn check_published(expect: &RefExpect, digest: &[u8; 32], wav: &[u8]) -> Result<()> {
    if sha::hex(digest) != expect.sha256 {
        return Err(Error::Refused(
            "reference sha256 does not match the published clip".into(),
        ));
    }
    let facts = wav::facts(wav)?;
    if facts.sample_rate != expect.sample_rate
        || facts.channels != expect.channels
        || facts.bits != expect.bits
    {
        return Err(Error::Refused(format!(
            "reference is {} Hz {} ch {} bit; published clip is {} Hz {} ch {} bit",
            facts.sample_rate,
            facts.channels,
            facts.bits,
            expect.sample_rate,
            expect.channels,
            expect.bits
        )));
    }
    Ok(())
}

fn header_for(persona: PersonaId, wav_bytes: &[u8], digest: [u8; 32]) -> Result<ClipHeader> {
    let facts = wav::facts(wav_bytes)?;
    let texture = persona.texture();
    Ok(ClipHeader {
        persona,
        texture,
        texture_strip: 0,
        texture_bits: texture.bits(),
        channels: facts.channels,
        bits: facts.bits,
        sample_rate: facts.sample_rate,
        audio_format: facts.audio_format,
        payload_len: wav_bytes.len() as u64,
        duration_ms: facts.duration_ms,
        payload_sha256: digest,
    })
}

fn write_file(path: &Path, header: &ClipHeader, payload: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(&parent.display().to_string(), e))?;
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&encode_header(header));
    out.extend_from_slice(payload);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &out).map_err(|e| Error::io(&tmp.display().to_string(), e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    Ok(())
}

fn encode_header(h: &ClipHeader) -> [u8; HEADER_LEN] {
    let mut b = [0u8; HEADER_LEN];
    b[0..8].copy_from_slice(&MAGIC);
    b[8..10].copy_from_slice(&VERSION.to_le_bytes());
    b[12..14].copy_from_slice(&h.persona.code().to_le_bytes());
    b[14..16].copy_from_slice(&h.texture.code().to_le_bytes());
    b[16..18].copy_from_slice(&h.texture_strip.to_le_bytes());
    b[18..20].copy_from_slice(&h.texture_bits.to_le_bytes());
    b[20..22].copy_from_slice(&h.channels.to_le_bytes());
    b[22..24].copy_from_slice(&h.bits.to_le_bytes());
    b[24..28].copy_from_slice(&h.sample_rate.to_le_bytes());
    b[28..30].copy_from_slice(&h.audio_format.to_le_bytes());
    b[32..40].copy_from_slice(&h.payload_len.to_le_bytes());
    b[40..48].copy_from_slice(&h.duration_ms.to_le_bytes());
    b[48..80].copy_from_slice(&h.payload_sha256);
    b
}

pub fn open(path: &Path) -> Result<Clip> {
    let meta = std::fs::metadata(path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    if meta.len() < HEADER_LEN as u64 || meta.len() > HEADER_LEN as u64 + MAX_WAV {
        return Err(Error::Bad(format!("{}: clip size", path.display())));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    parse(&bytes)
}

pub fn parse(bytes: &[u8]) -> Result<Clip> {
    if bytes.len() < HEADER_LEN || &bytes[0..8] != MAGIC {
        return Err(Error::Bad("clip: bad magic".into()));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(Error::Bad(format!("clip: version {version}")));
    }
    let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
    if flags != 0 {
        return Err(Error::Bad(format!("clip: flags {flags}")));
    }
    let persona = PersonaId::from_code(u16::from_le_bytes([bytes[12], bytes[13]]))?;
    let texture = TexturePolicy::from_code(u16::from_le_bytes([bytes[14], bytes[15]]))?;
    let texture_strip = u16::from_le_bytes([bytes[16], bytes[17]]);
    if texture_strip != 0 {
        return Err(Error::Refused("clip: texture_strip must be 0".into()));
    }
    let texture_bits = u16::from_le_bytes([bytes[18], bytes[19]]);
    if texture_bits != texture.bits() || texture != persona.texture() {
        return Err(Error::Bad(
            "clip: texture bits do not match the persona".into(),
        ));
    }
    let channels = u16::from_le_bytes([bytes[20], bytes[21]]);
    let bits = u16::from_le_bytes([bytes[22], bytes[23]]);
    let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let audio_format = u16::from_le_bytes([bytes[28], bytes[29]]);
    let reserved = u16::from_le_bytes([bytes[30], bytes[31]]);
    if reserved != 0 || bytes[80..112].iter().any(|&x| x != 0) {
        return Err(Error::Bad("clip: reserved bytes".into()));
    }
    let payload_len = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let duration_ms = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
    let mut payload_sha256 = [0u8; 32];
    payload_sha256.copy_from_slice(&bytes[48..80]);
    if payload_len as usize != bytes.len() - HEADER_LEN {
        return Err(Error::Bad("clip: payload length".into()));
    }
    let payload = bytes[HEADER_LEN..].to_vec();
    if sha256_check(&payload) != payload_sha256 {
        return Err(Error::Bad("clip: payload sha256".into()));
    }
    let facts = wav::facts(&payload)?;
    if facts.channels != channels
        || facts.bits != bits
        || facts.sample_rate != sample_rate
        || facts.audio_format != audio_format
        || facts.duration_ms != duration_ms
        || payload.len() as u64 != payload_len
    {
        return Err(Error::Bad("clip: header does not match payload".into()));
    }
    Ok(Clip {
        header: ClipHeader {
            persona,
            texture,
            texture_strip,
            texture_bits,
            channels,
            bits,
            sample_rate,
            audio_format,
            payload_len,
            duration_ms,
            payload_sha256,
        },
        payload,
    })
}

fn sha256_check(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_and_extra_chunk_survive() {
        let dir = std::env::temp_dir().join(format!("galion-clip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let samples = [20_000i16, 0, 0, -3];
        let bed = b"clicks,alarms,ambience";
        let raw = wav::encode_pcm16(48_000, 2, &samples, &[(*b"hub ", bed.as_slice())]).unwrap();
        let out = dir.join("clip.gclip");
        wrap_bytes(WrapBytes {
            persona: PersonaId::Jarvis,
            wav: &raw,
            out: &out,
            check_pin: false,
        })
        .unwrap();
        let pinned = wrap_bytes(WrapBytes {
            persona: PersonaId::Jarvis,
            wav: &raw,
            out: &out,
            check_pin: true,
        });
        assert!(
            pinned.is_err(),
            "a synthetic buffer is not the published clip"
        );
        let clip = open(&out).unwrap();
        assert_eq!(clip.payload(), raw.as_slice());
        assert_eq!(clip.header.channels, 2);
        assert_eq!(clip.header.sample_rate, 48_000);
        assert_eq!(clip.header.texture_strip, 0);
        assert_eq!(clip.header.texture, TexturePolicy::Keep);
        assert!(clip.payload().windows(bed.len()).any(|w| w == bed));
        let pcm = clip.interleaved_i16().unwrap();
        assert_eq!(pcm, samples);
        assert_ne!(pcm[0], pcm[1], "left and right stay distinct");
        let mut tampered = std::fs::read(&out).unwrap();
        tampered[HEADER_LEN] ^= 0xff;
        assert!(parse(&tampered).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
