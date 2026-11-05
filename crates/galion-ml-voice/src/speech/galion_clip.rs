// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Galion Voice Clip (`.gvc`). The clone landing emit.
//!
//! This is Galion's own container for one landed utterance. It is not a
//! Chatterbox safetensors pack, not a `.gq` weight file (`GALIONGQ` stays the
//! tensor reader), and not GGUF. Chatterbox, when it runs, stays in the other
//! process. What crosses back is PCM plus this clip.
//!
//! Jarvis hub texture is a separate layer. The layer record is always written
//! for Jarvis (`stripped = 0`). `enabled` is the toggle. Off does not delete
//! the layer and does not denoise the reference.
//!
//! Layout, little-endian, version 1:
//!
//! ```text
//! [0..8)    magic b"GALIONVC"
//! [8..10)   version u16 = 1
//! [10..12)  flags u16 (bit0 present, bit1 enabled, bit2 denoise-forbidden)
//! [12..16)  sample_rate u32
//! [16..18)  channels u16
//! [18..20)  bits u16
//! [20]      persona u8 (1 samantha, 2 jarvis)
//! [21]      model u8 (1 nano)
//! [22]      texture_present u8
//! [23]      texture_enabled u8
//! [24]      stripped u8 (must be 0)
//! [25]      denoise u8 (must be 0)
//! [26..28)  reserved
//! [28..32)  pcm_len u32
//! [32..36)  text_len u32
//! [36..40)  layer_len u32
//! [40..72)  reference sha256 (32 bytes; zero if the pin is unknown)
//! [72..80)  reserved
//! [80..)    text, then s16le PCM, then the texture layer record
//! ```
//!
//! The layer record is 64 bytes when present: `jarvis-hub`, enabled, stripped,
//! the reference sha256, and the policy `preserve-hub`. No filtered stem is
//! stored. The reference wav is not rewritten.

use super::clone_worker::{persona_name, ref_sha, CLONE_RATE_HZ};
use crate::VoiceError;

pub const MAGIC: &[u8; 8] = b"GALIONVC";
pub const VERSION: u16 = 1;
const HEADER_LEN: usize = 80;
const LAYER_LEN: usize = 64;
const FLAG_PRESENT: u16 = 1;
const FLAG_ENABLED: u16 = 2;
const FLAG_DENOISE_FORBIDDEN: u16 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureLayer {
    pub id: String,
    pub source_sha256: String,
    /// Always false. A true value is a broken clip.
    pub stripped: bool,
    pub enabled: bool,
    pub policy: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalionClip {
    pub persona: String,
    pub model: String,
    pub sample_rate: u32,
    pub text: String,
    pub pcm: Vec<i16>,
    pub reference_sha256: String,
    pub texture: Option<TextureLayer>,
}

/// Build the landing clip for one utterance.
///
/// Jarvis always gets the hub layer. `texture_enabled` is ignored for
/// Samantha. The PCM is stored as spoken; this function does not filter it.
pub fn emit_landing(persona: &str, text: &str, pcm: &[i16], texture_enabled: bool) -> Result<Vec<u8>, VoiceError> {
    let persona = persona_name(persona);
    if persona != "samantha" && persona != "jarvis" {
        return Err(VoiceError::Backend("unknown persona for galion clip".into()));
    }
    let sha = ref_sha(persona).unwrap_or_default();
    let layer = persona == "jarvis";
    let enabled = layer && texture_enabled;
    let mut flags = FLAG_DENOISE_FORBIDDEN;
    if layer {
        flags |= FLAG_PRESENT;
    }
    if enabled {
        flags |= FLAG_ENABLED;
    }
    let text_bytes = text.as_bytes();
    if text_bytes.len() > u32::MAX as usize || pcm.len() > (u32::MAX as usize) / 2 {
        return Err(VoiceError::Backend("galion clip too large".into()));
    }
    let layer_len = if layer { LAYER_LEN } else { 0 };
    let mut out = Vec::with_capacity(HEADER_LEN + text_bytes.len() + pcm.len() * 2 + layer_len);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&CLONE_RATE_HZ.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.push(if persona == "jarvis" { 2 } else { 1 });
    out.push(1); // nano. The landing model. Not a swap.
    out.push(u8::from(layer));
    out.push(u8::from(enabled));
    out.push(0); // stripped
    out.push(0); // denoise
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(pcm.len() as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(text_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&(layer_len as u32).to_le_bytes());
    out.extend_from_slice(&sha_bytes(&sha));
    out.extend_from_slice(&[0u8; 8]);
    debug_assert_eq!(out.len(), HEADER_LEN);
    out.extend_from_slice(text_bytes);
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    if layer {
        out.extend_from_slice(&layer_record(&sha, enabled));
    }
    Ok(out)
}

pub fn read_clip(bytes: &[u8]) -> Result<GalionClip, VoiceError> {
    if bytes.len() < HEADER_LEN || &bytes[0..8] != MAGIC {
        return Err(VoiceError::Backend("not a galion voice clip".into()));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(VoiceError::Backend(format!("galion clip version {version}")));
    }
    let rate = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let persona = match bytes[20] {
        1 => "samantha",
        2 => "jarvis",
        _ => return Err(VoiceError::Backend("galion clip persona".into())),
    };
    let model = match bytes[21] {
        1 => "nano",
        _ => return Err(VoiceError::Backend("galion clip model is not nano".into())),
    };
    let present = bytes[22] == 1;
    let enabled = bytes[23] == 1;
    if bytes[24] != 0 {
        return Err(VoiceError::Backend("texture layer was stripped".into()));
    }
    if bytes[25] != 0 {
        return Err(VoiceError::Backend("galion clip has denoise set".into()));
    }
    let pcm_len = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
    let text_len = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
    let layer_len = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
    let sha = hex_bytes(&bytes[40..72]);
    let rest = HEADER_LEN + text_len + pcm_len + layer_len;
    if bytes.len() < rest || pcm_len % 2 != 0 {
        return Err(VoiceError::Backend("galion clip truncated".into()));
    }
    if present != (layer_len == LAYER_LEN) {
        return Err(VoiceError::Backend("texture layer length does not match the flag".into()));
    }
    if persona == "jarvis" && !present {
        return Err(VoiceError::Backend("jarvis clip is missing the hub layer".into()));
    }
    if persona != "jarvis" && present {
        return Err(VoiceError::Backend("hub layer on a non-jarvis clip".into()));
    }
    let text = std::str::from_utf8(&bytes[HEADER_LEN..HEADER_LEN + text_len])
        .map_err(|_| VoiceError::Backend("galion clip text".into()))?
        .to_string();
    let pcm_bytes = &bytes[HEADER_LEN + text_len..HEADER_LEN + text_len + pcm_len];
    let pcm = pcm_bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    let texture = if present {
        let layer = &bytes[HEADER_LEN + text_len + pcm_len..rest];
        Some(read_layer(layer, enabled, &sha)?)
    } else {
        None
    };
    if enabled && texture.as_ref().is_none_or(|t| !t.enabled) {
        return Err(VoiceError::Backend("texture toggle does not match the layer".into()));
    }
    Ok(GalionClip {
        persona: persona.to_string(),
        model: model.to_string(),
        sample_rate: rate,
        text,
        pcm,
        reference_sha256: sha,
        texture,
    })
}

fn layer_record(sha: &str, enabled: bool) -> [u8; LAYER_LEN] {
    let mut rec = [0u8; LAYER_LEN];
    let id = b"jarvis-hub";
    rec[..id.len()].copy_from_slice(id);
    rec[16] = u8::from(enabled);
    rec[17] = 0;
    rec[20..52].copy_from_slice(&sha_bytes(sha));
    let policy = b"preserve-hub";
    rec[52..52 + policy.len()].copy_from_slice(policy);
    rec
}

fn read_layer(layer: &[u8], enabled_flag: bool, sha: &str) -> Result<TextureLayer, VoiceError> {
    if layer.len() != LAYER_LEN || &layer[..10] != b"jarvis-hub" {
        return Err(VoiceError::Backend("texture layer id".into()));
    }
    if layer[17] != 0 {
        return Err(VoiceError::Backend("texture layer was stripped".into()));
    }
    let enabled = layer[16] == 1;
    if enabled != enabled_flag {
        return Err(VoiceError::Backend("texture toggle does not match the layer".into()));
    }
    let layer_sha = hex_bytes(&layer[20..52]);
    if !sha.is_empty() && layer_sha != sha {
        return Err(VoiceError::Backend("texture layer sha drifted".into()));
    }
    let policy = std::str::from_utf8(&layer[52..64])
        .unwrap_or("")
        .trim_end_matches('\0')
        .to_string();
    if policy != "preserve-hub" {
        return Err(VoiceError::Backend("texture policy is not preserve-hub".into()));
    }
    Ok(TextureLayer {
        id: "jarvis-hub".into(),
        source_sha256: layer_sha,
        stripped: false,
        enabled,
        policy,
    })
}

fn sha_bytes(hex_str: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if hex_str.len() != 64 {
        return out;
    }
    for i in 0..32 {
        let byte = u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16).unwrap_or(0);
        out[i] = byte;
    }
    out
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `true` when the Chatterbox exception allows reference clips.
/// The landing does not write `LOCKS.toml`. A false result is
/// `BLOCKED-on-owner` for the lock flip.
pub fn reference_clips_allowed(locks_text: &str) -> bool {
    let mut in_section = false;
    for line in locks_text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.starts_with("[formats.exceptions.v2s14_chatterbox]");
            continue;
        }
        if !in_section || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() == "reference_clips" {
            return value.trim().starts_with("true");
        }
    }
    false
}

pub fn lock_flip_status(allowed: bool) -> &'static str {
    if allowed {
        "OPEN"
    } else {
        "BLOCKED-on-owner"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jarvis_layer_roundtrips_and_the_off_toggle_does_not_strip_it() {
        let pcm = [1i16, -2, 3];
        let on = emit_landing("jarvis", "Status.", &pcm, true).unwrap();
        assert!(on.starts_with(b"GALIONVC"));
        assert!(!on.windows(4).any(|w| w == b"GGUF"));
        assert!(!on.starts_with(b"GALIONGQ"));
        let clip = read_clip(&on).unwrap();
        assert_eq!(clip.persona, "jarvis");
        assert_eq!(clip.model, "nano");
        assert_eq!(clip.sample_rate, 24_000);
        assert_eq!(clip.pcm, pcm);
        assert_eq!(
            clip.reference_sha256,
            "60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6"
        );
        let layer = clip.texture.unwrap();
        assert_eq!(layer.id, "jarvis-hub");
        assert!(layer.enabled);
        assert!(!layer.stripped);
        assert_eq!(layer.policy, "preserve-hub");

        let off = emit_landing("jarvis", "Status.", &pcm, false).unwrap();
        let quiet = read_clip(&off).unwrap();
        let layer = quiet.texture.expect("off still keeps the layer");
        assert!(!layer.enabled);
        assert!(!layer.stripped);
        assert_eq!(layer.source_sha256, clip.reference_sha256);
        assert_eq!(quiet.pcm, pcm);
    }

    #[test]
    fn samantha_has_no_hub_layer_and_a_stripped_flag_is_refused() {
        let bytes = emit_landing("samantha", "Hi.", &[4, 5], true).unwrap();
        let clip = read_clip(&bytes).unwrap();
        assert!(clip.texture.is_none());
        assert_eq!(clip.persona, "samantha");
        assert_eq!(
            clip.reference_sha256,
            "1636428272b85d4b4f6c67bb859bfe33d9983ff9439646519305ff13e6921005"
        );
        let mut broken = bytes.clone();
        // Samantha header stripped byte is index 24.
        broken[24] = 1;
        assert!(read_clip(&broken).unwrap_err().to_string().contains("stripped"));
    }

    #[test]
    fn lock_flip_is_open_after_owner_t623u() {
        let locks = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml"),
        )
        .unwrap();
        assert!(reference_clips_allowed(&locks), "t623u opened the lock for the CC0 donors");
        assert_eq!(lock_flip_status(false), "BLOCKED-on-owner");
        assert_eq!(lock_flip_status(true), "OPEN");
        let opened = "[formats.exceptions.v2s14_chatterbox]\nreference_clips = true\n";
        assert!(reference_clips_allowed(opened));
    }
}
