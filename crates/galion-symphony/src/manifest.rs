// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `SYMPHONY.txt` — the pack's own text record. Not JSON, not a weight file.

use crate::cpu::{self, HostClass, HostId, VoiceBudget};
use crate::persona::{PersonaId, Prosody, TexturePolicy};
use crate::{Error, Result, BANDS, CLOCK_MS, FRAME_SAMPLES, LANE_HZ, PRESENCE_SCHEMA};
use std::path::Path;

pub const FILE_NAME: &str = "SYMPHONY.txt";

#[derive(Debug, Clone, PartialEq)]
pub struct TensorRec {
    pub name: String,
    pub file: String,
    pub dtype: String,
    pub shape: Vec<u64>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRec {
    pub rel: String,
    pub sha256: String,
    pub tensors: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClipRec {
    pub rel: String,
    pub file_sha256: String,
    pub payload_sha256: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub duration_ms: u64,
    pub texture: TexturePolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub persona: PersonaId,
    pub model: crate::persona::ModelId,
    pub host: HostClass,
    pub budget: VoiceBudget,
    pub texture: TexturePolicy,
    pub prosody: [Prosody; 2],
    pub emotions: Vec<String>,
    pub tags: Vec<String>,
    pub tags_enabled: bool,
    pub files: Vec<FileRec>,
    pub tensors: Vec<TensorRec>,
    pub clip: Option<ClipRec>,
}

pub fn token(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

pub fn untoken(s: &str) -> Result<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return Err(Error::Bad("token escape".into()));
            }
            let hex = std::str::from_utf8(&b[i + 1..i + 3])
                .map_err(|_| Error::Bad("token escape".into()))?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| Error::Bad("token escape".into()))?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| Error::Bad("token utf8".into()))
}

fn shape_str(dims: &[u64]) -> String {
    if dims.is_empty() {
        "scalar".into()
    } else {
        dims.iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("x")
    }
}

fn parse_shape(s: &str) -> Result<Vec<u64>> {
    if s == "scalar" {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for p in s.split('x') {
        out.push(p.parse().map_err(|_| Error::Bad(format!("shape {s}")))?);
    }
    Ok(out)
}

fn join_list(items: &[String]) -> String {
    if items.is_empty() {
        "-".into()
    } else {
        items.join(",")
    }
}

fn split_list(s: &str) -> Vec<String> {
    if s == "-" || s.is_empty() {
        Vec::new()
    } else {
        s.split(',').map(|p| p.to_string()).collect()
    }
}

impl Manifest {
    pub fn write(&self, dir: &Path) -> Result<()> {
        let text = self.render();
        if text.to_ascii_lowercase().contains("engagement") {
            return Err(Error::Bad(
                "manifest must not carry an engagement field".into(),
            ));
        }
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, text).map_err(|e| Error::io(&path.display().to_string(), e))
    }

    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push("GALION-SYMPHONY 1".into());
        lines.push("device cpu".into());
        lines.push("container gq".into());
        lines.push("source interchange-safetensors".into());
        lines.push("clip_abi 1".into());
        lines.push(format!("persona {}", self.persona.as_str()));
        lines.push(format!("model {}", self.model.as_str()));
        lines.push(format!("host {}", self.host.id.as_str()));
        lines.push(format!("clock_ms {CLOCK_MS}"));
        lines.push(format!("sample_rate {LANE_HZ}"));
        lines.push(format!("frame_samples {FRAME_SAMPLES}"));
        lines.push("pcm s16le".into());
        lines.push("empathy live-oklch".into());
        lines.push("scripted_empathy false".into());
        lines.push(format!("presence_schema {PRESENCE_SCHEMA}"));
        lines.push(format!("bands {}", BANDS.join(",")));
        lines.push("late_policy pause".into());
        lines.push(format!("texture_policy {}", self.texture.as_str()));
        let kinds: Vec<String> = self
            .texture
            .kinds()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        lines.push(format!("texture_kinds {}", join_list(&kinds)));
        lines.push("texture_strip false".into());
        lines.push(format!(
            "tags_enabled {}",
            if self.tags_enabled { "true" } else { "false" }
        ));
        lines.push(format!("tags {}", join_list(&self.tags)));
        for p in &self.prosody {
            lines.push(p.line());
        }
        lines.push(format!("emotions {}", join_list(&self.emotions)));
        lines.push(self.budget.line());
        lines.push(self.host.line());
        match &self.clip {
            None => lines.push("clip absent".into()),
            Some(c) => lines.push(format!(
                "clip {} {} {} {} {} {} {} {}",
                c.rel,
                c.file_sha256,
                c.payload_sha256,
                c.sample_rate,
                c.channels,
                c.bits,
                c.duration_ms,
                c.texture.as_str()
            )),
        }
        for f in &self.files {
            lines.push(format!("gq {} {} {}", f.rel, f.sha256, f.tensors));
        }
        for t in &self.tensors {
            lines.push(format!(
                "tensor {} {} {} {} {}",
                t.name,
                t.file,
                t.dtype,
                shape_str(&t.shape),
                token(&t.source)
            ));
        }
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }
}

pub fn read(dir: &Path) -> Result<Manifest> {
    let path = dir.join(FILE_NAME);
    let text =
        std::fs::read_to_string(&path).map_err(|e| Error::io(&path.display().to_string(), e))?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Manifest> {
    if text.to_ascii_lowercase().contains("engagement") {
        return Err(Error::Refused(
            "presence record rejects an engagement field".into(),
        ));
    }
    let mut lines = text.lines().filter(|l| !l.is_empty());
    let magic = lines
        .next()
        .ok_or_else(|| Error::Bad("empty manifest".into()))?;
    if magic != "GALION-SYMPHONY 1" {
        return Err(Error::Bad("manifest magic".into()));
    }
    let mut persona = None;
    let mut model = None;
    let mut host = None;
    let mut budget = None;
    let mut texture = None;
    let mut kinds: Option<Vec<String>> = None;
    let mut emotions = None;
    let mut tags = None;
    let mut tags_enabled = None;
    let mut prosody: Vec<Prosody> = Vec::new();
    let mut files = Vec::new();
    let mut tensors = Vec::new();
    let mut clip = None;
    let mut seen = std::collections::BTreeSet::new();
    for line in lines {
        let mut parts = line.split_whitespace();
        let key = parts.next().ok_or_else(|| Error::Bad("blank".into()))?;
        let rest: Vec<&str> = parts.collect();
        let one = |seen: &mut std::collections::BTreeSet<String>, k: &str| -> Result<()> {
            if !seen.insert(k.to_string()) {
                Err(Error::Bad(format!("duplicate {k}")))
            } else {
                Ok(())
            }
        };
        match key {
            "device" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "cpu" {
                    return Err(Error::Refused("symphony device is cpu".into()));
                }
            }
            "container" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "gq" {
                    return Err(Error::Refused("symphony container is gq".into()));
                }
            }
            "source" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "interchange-safetensors" {
                    return Err(Error::Bad("source".into()));
                }
            }
            "clip_abi" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "1" {
                    return Err(Error::Bad("clip_abi".into()));
                }
            }
            "persona" => {
                one(&mut seen, key)?;
                persona = Some(
                    PersonaId::parse(rest.first().copied().unwrap_or(""))
                        .ok_or_else(|| Error::Bad("persona".into()))?,
                );
            }
            "model" => {
                one(&mut seen, key)?;
                model = Some(
                    crate::persona::ModelId::parse(rest.first().copied().unwrap_or(""))
                        .ok_or_else(|| Error::Bad("model".into()))?,
                );
            }
            "host" => {
                one(&mut seen, key)?;
                host = Some(
                    HostId::parse(rest.first().copied().unwrap_or(""))
                        .ok_or_else(|| Error::Bad("host".into()))?,
                );
            }
            "clock_ms" => {
                one(&mut seen, key)?;
                if rest.first().and_then(|s| s.parse::<u32>().ok()) != Some(CLOCK_MS) {
                    return Err(Error::Bad("clock_ms".into()));
                }
            }
            "sample_rate" => {
                one(&mut seen, key)?;
                if rest.first().and_then(|s| s.parse::<u32>().ok()) != Some(LANE_HZ) {
                    return Err(Error::Bad(
                        "sample_rate is the 24 kHz lane, not the clip rate".into(),
                    ));
                }
            }
            "frame_samples" => {
                one(&mut seen, key)?;
                if rest.first().and_then(|s| s.parse::<u32>().ok()) != Some(FRAME_SAMPLES) {
                    return Err(Error::Bad("frame_samples".into()));
                }
            }
            "pcm" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "s16le" {
                    return Err(Error::Bad("pcm".into()));
                }
            }
            "empathy" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "live-oklch" {
                    return Err(Error::Refused(
                        "empathy is the live weight, not a script".into(),
                    ));
                }
            }
            "scripted_empathy" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "false" {
                    return Err(Error::Refused("scripted empathy is refused".into()));
                }
            }
            "presence_schema" => {
                one(&mut seen, key)?;
                if rest.first().copied() != Some(PRESENCE_SCHEMA) {
                    return Err(Error::Bad("presence_schema".into()));
                }
            }
            "bands" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "you,me,feeling" {
                    return Err(Error::Bad("bands".into()));
                }
            }
            "late_policy" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "pause" {
                    return Err(Error::Bad("late_policy".into()));
                }
            }
            "texture_policy" => {
                one(&mut seen, key)?;
                texture = Some(TexturePolicy::parse(rest.first().copied().unwrap_or(""))?);
            }
            "texture_kinds" => {
                one(&mut seen, key)?;
                kinds = Some(split_list(rest.first().copied().unwrap_or("")));
            }
            "texture_strip" => {
                one(&mut seen, key)?;
                if rest.len() != 1 || rest[0] != "false" {
                    return Err(Error::Refused("texture_strip must stay false".into()));
                }
            }
            "tags_enabled" => {
                one(&mut seen, key)?;
                tags_enabled = Some(match rest.first().copied() {
                    Some("true") => true,
                    Some("false") => false,
                    _ => return Err(Error::Bad("tags_enabled".into())),
                });
            }
            "tags" => {
                one(&mut seen, key)?;
                tags = Some(split_list(rest.first().copied().unwrap_or("")));
            }
            "prosody" => {
                if rest.len() != 7 {
                    return Err(Error::Bad("prosody".into()));
                }
                let f = |i: usize| {
                    rest[i]
                        .parse::<f32>()
                        .map_err(|_| Error::Bad("prosody".into()))
                };
                let level = rest[0];
                if level != "e0" && level != "e1" {
                    return Err(Error::Bad("prosody level".into()));
                }
                prosody.push(Prosody {
                    level: if level == "e0" { "e0" } else { "e1" },
                    pitch_st: f(1)?,
                    pitch_range: f(2)?,
                    rate: f(3)?,
                    loudness: f(4)?,
                    warmth: f(5)?,
                    pre_pause_ms: rest[6]
                        .parse()
                        .map_err(|_| Error::Bad("prosody pause".into()))?,
                });
            }
            "emotions" => {
                one(&mut seen, key)?;
                emotions = Some(split_list(rest.first().copied().unwrap_or("")));
            }
            "budget" => {
                one(&mut seen, key)?;
                budget = Some(VoiceBudget::parse(&rest.join(" "))?);
            }
            "host_spec" => {
                one(&mut seen, key)?;
                if rest.len() != 8 {
                    return Err(Error::Bad("host_spec".into()));
                }
                let id = HostId::parse(rest[0]).ok_or_else(|| Error::Bad("host_spec id".into()))?;
                let spec = id.spec();
                if rest[1] != spec.role {
                    return Err(Error::Bad("host_spec role".into()));
                }
                if cpu::parse_opt(rest[2])? != spec.cores
                    || cpu::parse_opt(rest[3])? != spec.threads
                    || cpu::parse_opt(rest[4])? != spec.ram_gb
                    || cpu::parse_opt(rest[5])? != spec.ssd_gb
                    || cpu::parse_opt(rest[6])? != spec.mem_bw_gbps
                    || cpu::parse_opt(rest[7])? != spec.price_usd_mo
                {
                    return Err(Error::Bad(
                        "host_spec does not match the known class".into(),
                    ));
                }
            }
            "clip" => {
                one(&mut seen, key)?;
                if rest.len() == 1 && rest[0] == "absent" {
                    clip = None;
                } else if rest.len() == 8 {
                    clip = Some(ClipRec {
                        rel: rest[0].to_string(),
                        file_sha256: rest[1].to_string(),
                        payload_sha256: rest[2].to_string(),
                        sample_rate: rest[3]
                            .parse()
                            .map_err(|_| Error::Bad("clip rate".into()))?,
                        channels: rest[4]
                            .parse()
                            .map_err(|_| Error::Bad("clip channels".into()))?,
                        bits: rest[5]
                            .parse()
                            .map_err(|_| Error::Bad("clip bits".into()))?,
                        duration_ms: rest[6]
                            .parse()
                            .map_err(|_| Error::Bad("clip duration".into()))?,
                        texture: TexturePolicy::parse(rest[7])?,
                    });
                } else {
                    return Err(Error::Bad("clip".into()));
                }
            }
            "gq" => {
                if rest.len() != 3 {
                    return Err(Error::Bad("gq".into()));
                }
                files.push(FileRec {
                    rel: rest[0].into(),
                    sha256: rest[1].into(),
                    tensors: rest[2].parse().map_err(|_| Error::Bad("gq count".into()))?,
                });
            }
            "tensor" => {
                if rest.len() != 5 {
                    return Err(Error::Bad("tensor".into()));
                }
                if !rest[0].starts_with("sym.") {
                    return Err(Error::Refused(format!(
                        "tensor {} is outside sym.",
                        rest[0]
                    )));
                }
                tensors.push(TensorRec {
                    name: rest[0].into(),
                    file: rest[1].into(),
                    dtype: rest[2].into(),
                    shape: parse_shape(rest[3])?,
                    source: untoken(rest[4])?,
                });
            }
            other => return Err(Error::Bad(format!("unknown manifest key {other}"))),
        }
    }
    for k in [
        "device",
        "container",
        "source",
        "clip_abi",
        "persona",
        "model",
        "host",
        "clock_ms",
        "sample_rate",
        "frame_samples",
        "pcm",
        "empathy",
        "scripted_empathy",
        "presence_schema",
        "bands",
        "late_policy",
        "texture_policy",
        "texture_kinds",
        "texture_strip",
        "tags_enabled",
        "tags",
        "emotions",
        "budget",
        "host_spec",
        "clip",
    ] {
        if !seen.contains(k) {
            return Err(Error::Bad(format!("missing {k}")));
        }
    }
    if prosody.len() != 2 || prosody[0].level != "e0" || prosody[1].level != "e1" {
        return Err(Error::Bad("prosody must be e0 then e1".into()));
    }
    let persona = persona.unwrap();
    let texture = texture.unwrap();
    if texture != persona.texture() {
        return Err(Error::Bad("texture policy does not match persona".into()));
    }
    let expect_kinds: Vec<String> = texture.kinds().iter().map(|s| (*s).to_string()).collect();
    if kinds.as_ref() != Some(&expect_kinds) {
        return Err(Error::Bad("texture kinds".into()));
    }
    let host = host.unwrap().spec();
    if files.is_empty() || tensors.is_empty() {
        return Err(Error::Bad("pack has no tensors".into()));
    }
    Ok(Manifest {
        persona,
        model: model.unwrap(),
        host,
        budget: budget.unwrap(),
        texture,
        prosody: [prosody[0], prosody[1]],
        emotions: emotions.unwrap(),
        tags: tags.unwrap(),
        tags_enabled: tags_enabled.unwrap(),
        files,
        tensors,
        clip,
    })
}
