// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Fixed stand-ins for the inner-life layer that is not built yet.
//!
//! Four files, one directory, schema `galion-emotion-standin/1`:
//! `persona.txt`, `memory.txt`, `inner-life.txt`, `intent.txt`.
//! Loading allocates. The frame step does not call this module.
//! Memory and inner life are readable context. They do not move the moment.
//! The intent script is the message stream that does.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::axes::{APPROACH, AROUSAL, AXES, DOMINANCE, VALENCE};
use crate::baseline::VoiceBaseline;
use crate::clock::FRAME_MS;
use crate::factors::BreathPhase;
use crate::intent::{
    pack_verb, IntentMsg, IntentSource, MASK_BREATH, MASK_ENERGY, MASK_FATIGUE, MASK_HEALTH,
};
use crate::state::EmotionCore;
use crate::CoreError;

pub const SCHEMA: &str = "galion-emotion-standin/1";
pub const FILE_PERSONA: &str = "persona.txt";
pub const FILE_MEMORY: &str = "memory.txt";
pub const FILE_INNER: &str = "inner-life.txt";
pub const FILE_INTENT: &str = "intent.txt";

const MAX_TEXT: usize = 240;
const MAX_MEMORY: usize = 16;
const MAX_FRAMES: u32 = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandinError {
    pub file: &'static str,
    pub line: u32,
    pub message: String,
}

impl std::fmt::Display for StandinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

impl std::error::Error for StandinError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryCard {
    pub id: String,
    pub snippet: String,
}

/// Cares, wants, expects. A fixture, not a model of a mind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InnerLifeStandin {
    pub care: String,
    pub want: String,
    pub expect: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StandinWorld {
    pub baseline: VoiceBaseline,
    pub memory: Vec<MemoryCard>,
    pub inner: InnerLifeStandin,
    pub script: Vec<IntentMsg>,
}

impl StandinWorld {
    pub fn replay(&self) -> Result<EmotionCore, CoreError> {
        EmotionCore::replay(self.baseline, &self.script)
    }
}

pub fn load_standins(dir: &Path) -> Result<StandinWorld, StandinError> {
    let persona = read_file(dir, FILE_PERSONA)?;
    let memory = read_file(dir, FILE_MEMORY)?;
    let inner = read_file(dir, FILE_INNER)?;
    let intent = read_file(dir, FILE_INTENT)?;
    Ok(StandinWorld {
        baseline: parse_persona(&persona)?,
        memory: parse_memory(&memory)?,
        inner: parse_inner(&inner)?,
        script: parse_intent(&intent)?,
    })
}

fn read_file(dir: &Path, name: &'static str) -> Result<String, StandinError> {
    let path = dir.join(name);
    fs::read_to_string(&path).map_err(|_| StandinError {
        file: name,
        line: 0,
        message: format!("cannot read {}", path.display()),
    })
}

pub fn parse_persona(text: &str) -> Result<VoiceBaseline, StandinError> {
    let records = parse_blocks(text, FILE_PERSONA, "persona")?;
    let rec = one(FILE_PERSONA, &records)?;
    let persona = id_slot(FILE_PERSONA, rec, "id")?;
    let voice = id_slot(FILE_PERSONA, rec, "voice")?;
    let mut pos = [0.0; AXES];
    pos[VALENCE] = num(FILE_PERSONA, rec, "valence", -1.0, 1.0)?;
    pos[AROUSAL] = num(FILE_PERSONA, rec, "arousal", 0.0, 1.0)?;
    pos[DOMINANCE] = num(FILE_PERSONA, rec, "dominance", 0.0, 1.0)?;
    pos[APPROACH] = num(FILE_PERSONA, rec, "approach", -1.0, 1.0)?;
    let pitch = num(FILE_PERSONA, rec, "pitch_centre_hz", 1.0, 2000.0)?;
    Ok(VoiceBaseline {
        persona,
        voice,
        pos,
        pitch_centre_hz: pitch,
    })
}

pub fn parse_memory(text: &str) -> Result<Vec<MemoryCard>, StandinError> {
    let records = parse_blocks(text, FILE_MEMORY, "memory")?;
    if records.is_empty() || records.len() > MAX_MEMORY {
        return err(FILE_MEMORY, 0, "memory needs 1 to 16 cards");
    }
    let mut out = Vec::with_capacity(records.len());
    for rec in &records {
        let id = text_field(FILE_MEMORY, rec, "id")?;
        let snippet = text_field(FILE_MEMORY, rec, "snippet")?;
        if id.len() > 32 {
            return err(FILE_MEMORY, rec.line, "memory id is longer than 32 bytes");
        }
        out.push(MemoryCard { id, snippet });
    }
    Ok(out)
}

pub fn parse_inner(text: &str) -> Result<InnerLifeStandin, StandinError> {
    let records = parse_blocks(text, FILE_INNER, "inner")?;
    let rec = one(FILE_INNER, &records)?;
    Ok(InnerLifeStandin {
        care: text_field(FILE_INNER, rec, "care")?,
        want: text_field(FILE_INNER, rec, "want")?,
        expect: text_field(FILE_INNER, rec, "expect")?,
    })
}

pub fn parse_intent(text: &str) -> Result<Vec<IntentMsg>, StandinError> {
    let records = parse_blocks(text, FILE_INTENT, "intent")?;
    if records.is_empty() {
        return err(FILE_INTENT, 0, "intent script is empty");
    }
    let mut script = Vec::new();
    let mut expect: u32 = 0;
    for rec in &records {
        let t0 = u32_field(FILE_INTENT, rec, "t_ms")?;
        if t0 != expect {
            return err(
                FILE_INTENT,
                rec.line,
                format!("t_ms {t0} is not the next frame {expect}"),
            );
        }
        let repeat = match rec.map.get("repeat") {
            None => 1u32,
            Some(s) => s.parse::<u32>().map_err(|_| StandinError {
                file: FILE_INTENT,
                line: rec.line,
                message: "repeat is not an integer".into(),
            })?,
        };
        if repeat == 0 || repeat > MAX_FRAMES {
            return err(FILE_INTENT, rec.line, "repeat must be 1..=4096");
        }
        let proto = intent_from_record(rec)?;
        for i in 0..repeat {
            if script.len() as u32 >= MAX_FRAMES {
                return err(FILE_INTENT, rec.line, "script is longer than 4096 frames");
            }
            let t = t0
                .checked_add(i.checked_mul(FRAME_MS).ok_or_else(|| StandinError {
                    file: FILE_INTENT,
                    line: rec.line,
                    message: "t_ms overflow".into(),
                })?)
                .ok_or_else(|| StandinError {
                    file: FILE_INTENT,
                    line: rec.line,
                    message: "t_ms overflow".into(),
                })?;
            let mut msg = proto;
            msg.t_ms = t;
            script.push(msg);
            expect = expect.checked_add(FRAME_MS).ok_or_else(|| StandinError {
                file: FILE_INTENT,
                line: rec.line,
                message: "t_ms overflow".into(),
            })?;
        }
    }
    Ok(script)
}

fn intent_from_record(rec: &Record) -> Result<IntentMsg, StandinError> {
    let mut msg = IntentMsg::hold(0);
    if let Some(verb) = rec.map.get("verb") {
        msg.verb = pack_verb(verb).map_err(|_| StandinError {
            file: FILE_INTENT,
            line: rec.line,
            message: "verb must be 1..=16 printable ascii bytes".into(),
        })?;
        if verb.is_empty() {
            return err(FILE_INTENT, rec.line, "verb is empty");
        }
    }
    if let Some(source) = rec.map.get("source") {
        msg.source = match source.as_str() {
            "own" => IntentSource::Own,
            "inbound" => IntentSource::Inbound,
            _ => return err(FILE_INTENT, rec.line, "source must be own or inbound"),
        };
    }
    if let Some(consent) = rec.map.get("consent") {
        msg.consent = parse_consent(consent).ok_or_else(|| StandinError {
            file: FILE_INTENT,
            line: rec.line,
            message: "consent must be 0..=15 or sense,adapt,remember,share".into(),
        })?;
    }
    if let Some(strength) = rec.map.get("strength") {
        msg.strength = parse_f32(strength).ok_or_else(|| StandinError {
            file: FILE_INTENT,
            line: rec.line,
            message: "strength is not a number".into(),
        })?;
    }
    let pulls = [
        ("valence_pull", VALENCE),
        ("arousal_pull", AROUSAL),
        ("dominance_pull", DOMINANCE),
        ("approach_pull", APPROACH),
    ];
    for (key, axis) in pulls {
        if let Some(v) = rec.map.get(key) {
            let n = parse_f32(v).ok_or_else(|| StandinError {
                file: FILE_INTENT,
                line: rec.line,
                message: format!("{key} is not a number"),
            })?;
            msg.pull[axis] = n;
            msg.pull_mask |= 1 << axis;
        }
    }
    if let Some(v) = rec.map.get("energy") {
        msg.energy = must_f32(rec, "energy", v)?;
        msg.factor_mask |= MASK_ENERGY;
    }
    if let Some(v) = rec.map.get("fatigue") {
        msg.fatigue = must_f32(rec, "fatigue", v)?;
        msg.factor_mask |= MASK_FATIGUE;
    }
    if let Some(v) = rec.map.get("health") {
        msg.health = must_f32(rec, "health", v)?;
        msg.factor_mask |= MASK_HEALTH;
    }
    if rec.map.contains_key("breath")
        || rec.map.contains_key("volume")
        || rec.map.contains_key("flow")
    {
        let phase = rec.map.get("breath").map(|s| s.as_str()).unwrap_or("rest");
        msg.breath_phase = BreathPhase::parse(phase).ok_or_else(|| StandinError {
            file: FILE_INTENT,
            line: rec.line,
            message: "breath must be rest, inhale, hold, speech, or exhale".into(),
        })?;
        if let Some(v) = rec.map.get("volume") {
            msg.lung_volume = must_f32(rec, "volume", v)?;
        }
        if let Some(v) = rec.map.get("flow") {
            msg.air_flow = must_f32(rec, "flow", v)?;
        }
        msg.factor_mask |= MASK_BREATH;
    }
    crate::intent::validate(&msg).map_err(|_| StandinError {
        file: FILE_INTENT,
        line: rec.line,
        message: "intent frame is outside its ranges".into(),
    })?;
    Ok(msg)
}

fn must_f32(rec: &Record, key: &str, v: &str) -> Result<f32, StandinError> {
    parse_f32(v).ok_or_else(|| StandinError {
        file: FILE_INTENT,
        line: rec.line,
        message: format!("{key} is not a number"),
    })
}

fn parse_consent(s: &str) -> Option<u8> {
    if let Ok(n) = s.parse::<u8>() {
        return if n <= 0x0f { Some(n) } else { None };
    }
    let mut bits = 0u8;
    if s.is_empty() {
        return None;
    }
    for part in s.split(',') {
        let part = part.trim();
        bits |= match part {
            "sense" => 1 << 0,
            "adapt" => 1 << 1,
            "remember" => 1 << 2,
            "share" => 1 << 3,
            _ => return None,
        };
    }
    Some(bits)
}

struct Record {
    line: u32,
    map: BTreeMap<String, String>,
}

fn one<'a>(file: &'static str, records: &'a [Record]) -> Result<&'a Record, StandinError> {
    if records.len() != 1 {
        return err(file, 0, "expected one record");
    }
    Ok(&records[0])
}

fn id_slot(file: &'static str, rec: &Record, key: &str) -> Result<[u8; 16], StandinError> {
    let text = req(file, rec, key)?;
    if text.is_empty()
        || text.len() > 16
        || !text.is_ascii()
        || text.bytes().any(|b| !(0x20..=0x7e).contains(&b))
    {
        return err(
            file,
            rec.line,
            format!("{key} must be 1..=16 printable ascii bytes"),
        );
    }
    let mut out = [0u8; 16];
    out[..text.len()].copy_from_slice(text.as_bytes());
    Ok(out)
}

fn num(file: &'static str, rec: &Record, key: &str, lo: f32, hi: f32) -> Result<f32, StandinError> {
    let text = req(file, rec, key)?;
    let v = parse_f32(text).ok_or_else(|| StandinError {
        file,
        line: rec.line,
        message: format!("{key} is not a number"),
    })?;
    if v < lo || v > hi {
        return err(file, rec.line, format!("{key} is outside {lo}..={hi}"));
    }
    Ok(v)
}

fn text_field(file: &'static str, rec: &Record, key: &str) -> Result<String, StandinError> {
    let text = req(file, rec, key)?;
    if text.is_empty() || text.len() > MAX_TEXT {
        return err(file, rec.line, format!("{key} must be 1..=240 bytes"));
    }
    Ok(text.to_string())
}

fn u32_field(file: &'static str, rec: &Record, key: &str) -> Result<u32, StandinError> {
    let text = req(file, rec, key)?;
    text.parse::<u32>().map_err(|_| StandinError {
        file,
        line: rec.line,
        message: format!("{key} is not an integer"),
    })
}

fn req<'a>(file: &'static str, rec: &'a Record, key: &str) -> Result<&'a str, StandinError> {
    rec.map
        .get(key)
        .map(|s| s.as_str())
        .ok_or_else(|| StandinError {
            file,
            line: rec.line,
            message: format!("missing {key}"),
        })
}

fn err<T>(file: &'static str, line: u32, message: impl Into<String>) -> Result<T, StandinError> {
    Err(StandinError {
        file,
        line,
        message: message.into(),
    })
}

fn parse_f32(s: &str) -> Option<f32> {
    let v: f32 = s.parse().ok()?;
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn parse_blocks(text: &str, file: &'static str, kind: &str) -> Result<Vec<Record>, StandinError> {
    let mut blocks: Vec<Vec<(u32, String, String)>> = vec![Vec::new()];
    for (idx, raw) in text.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "---" {
            blocks.push(Vec::new());
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if key.is_empty() || value.is_empty() {
            return err(file, line_no, "expected `key value`");
        }
        blocks
            .last_mut()
            .unwrap()
            .push((line_no, key.to_string(), value.to_string()));
    }
    if blocks.first().map(|b| b.is_empty()).unwrap_or(true) {
        return err(file, 0, "missing schema");
    }
    let header = &blocks[0];
    let schema = header
        .iter()
        .find(|(_, k, _)| k == "schema")
        .map(|(_, _, v)| v.as_str());
    let got_kind = header
        .iter()
        .find(|(_, k, _)| k == "kind")
        .map(|(_, _, v)| v.as_str());
    if schema != Some(SCHEMA) {
        return err(file, header[0].0, format!("schema must be {SCHEMA}"));
    }
    if got_kind != Some(kind) {
        return err(file, header[0].0, format!("kind must be {kind}"));
    }
    let allowed: &[&str] = match kind {
        "persona" => &[
            "id",
            "voice",
            "valence",
            "arousal",
            "dominance",
            "approach",
            "pitch_centre_hz",
        ],
        "memory" => &["id", "snippet"],
        "inner" => &["care", "want", "expect"],
        "intent" => &[
            "t_ms",
            "repeat",
            "verb",
            "source",
            "consent",
            "strength",
            "valence_pull",
            "arousal_pull",
            "dominance_pull",
            "approach_pull",
            "energy",
            "fatigue",
            "health",
            "breath",
            "volume",
            "flow",
        ],
        _ => return err(file, 0, "unknown kind"),
    };
    let mut records = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let mut map = BTreeMap::new();
        let mut line = 0u32;
        for (line_no, key, value) in block {
            line = *line_no;
            if key == "schema" || key == "kind" {
                if i != 0 {
                    return err(file, *line_no, "schema and kind belong on the first block");
                }
                continue;
            }
            if !allowed.iter().any(|name| name == key) {
                return err(file, *line_no, format!("unknown key {key}"));
            }
            if map.insert(key.clone(), value.clone()).is_some() {
                return err(file, *line_no, format!("duplicate key {key}"));
            }
        }
        if map.is_empty() {
            continue;
        }
        records.push(Record { line, map });
    }
    Ok(records)
}
