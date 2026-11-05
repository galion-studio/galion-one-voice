// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Persona register carried by the symphony pack.
//!
//! Numbers are the draft tables in
//! `docs/galion-voice-v2/personas/voice/{samantha,jarvis}.toml`.
//! Emotions are the register in the persona pages. They are not new
//! affect-wire labels, and they are not a scripted empathy line.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonaId {
    Samantha,
    Jarvis,
}

impl PersonaId {
    pub fn as_str(self) -> &'static str {
        match self {
            PersonaId::Samantha => "samantha",
            PersonaId::Jarvis => "jarvis",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "samantha" => Some(PersonaId::Samantha),
            "jarvis" => Some(PersonaId::Jarvis),
            _ => None,
        }
    }

    /// Clip ABI persona code.
    pub fn code(self) -> u16 {
        match self {
            PersonaId::Samantha => 1,
            PersonaId::Jarvis => 2,
        }
    }

    pub fn from_code(c: u16) -> Result<Self> {
        match c {
            1 => Ok(PersonaId::Samantha),
            2 => Ok(PersonaId::Jarvis),
            _ => Err(Error::Bad(format!("clip persona code {c}"))),
        }
    }

    pub fn texture(self) -> TexturePolicy {
        match self {
            PersonaId::Samantha => TexturePolicy::Voice,
            PersonaId::Jarvis => TexturePolicy::Keep,
        }
    }

    pub fn prosody(self) -> [Prosody; 2] {
        match self {
            PersonaId::Samantha => [
                Prosody {
                    level: "e0",
                    pitch_st: 0.0,
                    pitch_range: 1.00,
                    rate: 1.00,
                    loudness: 1.00,
                    warmth: 0.62,
                    pre_pause_ms: 0,
                },
                Prosody {
                    level: "e1",
                    pitch_st: -0.5,
                    pitch_range: 0.88,
                    rate: 0.92,
                    loudness: 0.80,
                    warmth: 0.75,
                    pre_pause_ms: 250,
                },
            ],
            PersonaId::Jarvis => [
                Prosody {
                    level: "e0",
                    pitch_st: -0.3,
                    pitch_range: 0.92,
                    rate: 0.96,
                    loudness: 0.92,
                    warmth: 0.42,
                    pre_pause_ms: 80,
                },
                Prosody {
                    level: "e1",
                    pitch_st: -0.4,
                    pitch_range: 0.90,
                    rate: 0.94,
                    loudness: 0.88,
                    warmth: 0.55,
                    pre_pause_ms: 120,
                },
            ],
        }
    }

    pub fn emotions(self) -> &'static [&'static str] {
        match self {
            PersonaId::Samantha => &[
                "curiosity",
                "delight",
                "tenderness",
                "play",
                "wonder",
                "hurt",
            ],
            PersonaId::Jarvis => &["calm", "amusement", "focus", "satisfaction"],
        }
    }

    pub fn tags(self) -> &'static [&'static str] {
        match self {
            PersonaId::Samantha => &["laugh", "chuckle"],
            PersonaId::Jarvis => &[],
        }
    }

    pub fn tags_enabled(self) -> bool {
        self == PersonaId::Samantha
    }

    /// Published reference description. Checked only when a real clip is wrapped
    /// with pin checking on. The bytes are not in git.
    pub fn ref_expect(self) -> RefExpect {
        match self {
            PersonaId::Samantha => RefExpect {
                sha256: "1636428272b85d4b4f6c67bb859bfe33d9983ff9439646519305ff13e6921005",
                sample_rate: 44_100,
                channels: 1,
                bits: 16,
            },
            PersonaId::Jarvis => RefExpect {
                sha256: "60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6",
                sample_rate: 44_100,
                channels: 1,
                bits: 16,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexturePolicy {
    /// Keep the reference bed. Jarvis: clicks, alarms, ambience stay.
    Keep,
    /// Voice register. Still no strip step. Samantha has no hub bed to remove.
    Voice,
}

impl TexturePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            TexturePolicy::Keep => "keep",
            TexturePolicy::Voice => "voice",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "keep" => Ok(TexturePolicy::Keep),
            "voice" => Ok(TexturePolicy::Voice),
            _ => Err(Error::Bad(format!("texture policy {s}"))),
        }
    }

    pub fn code(self) -> u16 {
        match self {
            TexturePolicy::Keep => 1,
            TexturePolicy::Voice => 2,
        }
    }

    pub fn from_code(c: u16) -> Result<Self> {
        match c {
            1 => Ok(TexturePolicy::Keep),
            2 => Ok(TexturePolicy::Voice),
            _ => Err(Error::Bad(format!("texture policy code {c}"))),
        }
    }

    /// Bit 0 clicks, bit 1 alarms, bit 2 ambience.
    pub fn bits(self) -> u16 {
        match self {
            TexturePolicy::Keep => 0b111,
            TexturePolicy::Voice => 0,
        }
    }

    pub fn kinds(self) -> &'static [&'static str] {
        match self {
            TexturePolicy::Keep => &["clicks", "alarms", "ambience"],
            TexturePolicy::Voice => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prosody {
    pub level: &'static str,
    pub pitch_st: f32,
    pub pitch_range: f32,
    pub rate: f32,
    pub loudness: f32,
    pub warmth: f32,
    pub pre_pause_ms: u32,
}

impl Prosody {
    pub fn line(self) -> String {
        format!(
            "prosody {} {:.2} {:.2} {:.2} {:.2} {:.2} {}",
            self.level,
            self.pitch_st,
            self.pitch_range,
            self.rate,
            self.loudness,
            self.warmth,
            self.pre_pause_ms
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefExpect {
    pub sha256: &'static str,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelId {
    Nano,
    Turbo,
}

impl ModelId {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelId::Nano => "nano",
            ModelId::Turbo => "turbo",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "nano" => Some(ModelId::Nano),
            "turbo" => Some(ModelId::Turbo),
            _ => None,
        }
    }

    pub fn files(self) -> &'static [&'static str] {
        match self {
            ModelId::Nano => &[
                "shared/s3gen.safetensors",
                "shared/s3gen_meanflow.safetensors",
                "shared/ve.safetensors",
                "nano/t3_nano_v1.safetensors",
            ],
            ModelId::Turbo => &[
                "shared/s3gen.safetensors",
                "shared/s3gen_meanflow.safetensors",
                "shared/ve.safetensors",
                "turbo/t3_turbo_v1.safetensors",
            ],
        }
    }
}

pub fn sector(file_name: &str) -> Result<&'static str> {
    match file_name {
        "ve.safetensors" => Ok("voice_encoder"),
        "s3gen.safetensors" => Ok("speech_gen"),
        "s3gen_meanflow.safetensors" => Ok("speech_flow"),
        "t3_nano_v1.safetensors" => Ok("token_nano"),
        "t3_turbo_v1.safetensors" => Ok("token_turbo"),
        other => Err(Error::Refused(format!(
            "{other} is not a symphony interchange name"
        ))),
    }
}

pub fn galion_name(
    sector_name: &str,
    source: &str,
    used: &mut std::collections::BTreeSet<String>,
) -> Result<String> {
    let mut body = String::new();
    for c in source.chars() {
        if c.is_ascii_alphanumeric() {
            body.push(c);
        } else {
            body.push('_');
        }
    }
    while body.contains("__") {
        body = body.replace("__", "_");
    }
    let body = body.trim_matches('_');
    if body.is_empty() {
        return Err(Error::Bad(format!("empty tensor name from {source}")));
    }
    let mut name = format!("sym.{sector_name}.{body}");
    if name.len() > galion_tensor::gq::MAX_NAME_LEN {
        return Err(Error::Bad(format!(
            "tensor name longer than .gq allows: {source}"
        )));
    }
    if !used.insert(name.clone()) {
        let mut i = 2u32;
        loop {
            let n = format!("{name}_{i}");
            if used.insert(n.clone()) {
                name = n;
                break;
            }
            i += 1;
            if i > 10_000 {
                return Err(Error::Bad(format!("too many collisions for {name}")));
            }
        }
    }
    Ok(name)
}
