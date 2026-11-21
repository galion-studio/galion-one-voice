// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Wire types for the Samantha dot.
//!
//! Text frames are JSON objects with `schema` [`SCHEMA`].
//! Binary frames are [`PcmFrame`] (`GVPC`). The layout is
//! `PROTOCOL-ASSUMED.md`. HORBM is not in this crate. [`crate::engine::ReplyEngine`]
//! is the seam a later `Brain::turn` can fill.

use std::fmt;

/// Schema on every JSON frame this sector speaks.
pub const SCHEMA: &str = "galion-samantha-dot/1";

/// Mood and state clock. Same period as `galion-heartbeat` `FRAME_MS`.
pub const TICK_MS: u32 = 80;

/// Preferred capture rate on the page. 24 kHz is the heartbeat rate.
pub const PCM_RATE_HZ: u32 = 16_000;

/// Samples in one 80 ms frame at [`PCM_RATE_HZ`].
pub const PCM_FRAME_SAMPLES: usize = 1_280;

const PCM_MAGIC: &[u8; 4] = b"GVPC";
const MAX_SAMPLES: usize = 48_000;
const MAX_JSON: usize = 65_536;

/// Pleasure / arousal / dominance. Field order matches the HORBM `Pad`.
///
/// [`Pad::RESTING`] is the brain's rest pose. Valence follows the emotion
/// crate (about -1..=1). Arousal and dominance are clamped to 0..=1 for the
/// mark. Non-finite channels fall back to rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    pub fn sane(self) -> Pad {
        Pad {
            arousal: finite_unit(self.arousal, Self::RESTING.arousal, 0.0, 1.0),
            valence: finite_unit(self.valence, Self::RESTING.valence, -1.0, 1.0),
            dominance: finite_unit(self.dominance, Self::RESTING.dominance, 0.0, 1.0),
        }
    }
}

fn finite_unit(x: f32, fallback: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        fallback
    }
}

/// What the dot is doing. Thinking is a motion of the same mark.
/// `LOCKS.toml` `dot_states` on the Galion One page is still
/// idle / listening / speaking. This sector names thinking because the
/// existing phone page already draws it (`ORB.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceState {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

impl VoiceState {
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceState::Idle => "idle",
            VoiceState::Listening => "listening",
            VoiceState::Thinking => "thinking",
            VoiceState::Speaking => "speaking",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(VoiceState::Idle),
            "listening" => Some(VoiceState::Listening),
            "thinking" => Some(VoiceState::Thinking),
            "speaking" => Some(VoiceState::Speaking),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// First frame on a connection. Does not advance the clock.
    State,
    /// One 80 ms tick.
    Tick,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::State => "state",
            EventKind::Tick => "tick",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    pub text: String,
    pub sources: Vec<String>,
    pub final_: bool,
}

/// Absent keeps the caption the page already has.
/// `Clear` is JSON `null`. `Set` replaces it.
#[derive(Debug, Clone, PartialEq)]
pub enum CaptionSlot {
    Hold,
    Clear,
    Set(Caption),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerEvent {
    pub kind: EventKind,
    pub state: VoiceState,
    pub mood: Pad,
    pub level: f32,
    pub t_ms: u32,
    pub caption: CaptionSlot,
}

impl ServerEvent {
    pub fn to_json(&self) -> String {
        let mut s = String::with_capacity(192);
        s.push_str("{\"schema\":");
        push_str(&mut s, SCHEMA);
        s.push_str(",\"kind\":");
        push_str(&mut s, self.kind.as_str());
        s.push_str(",\"state\":");
        push_str(&mut s, self.state.as_str());
        s.push_str(",\"level\":");
        push_num(&mut s, self.level);
        s.push_str(",\"t_ms\":");
        s.push_str(&self.t_ms.to_string());
        s.push_str(",\"mood\":{\"arousal\":");
        push_num(&mut s, self.mood.arousal);
        s.push_str(",\"valence\":");
        push_num(&mut s, self.mood.valence);
        s.push_str(",\"dominance\":");
        push_num(&mut s, self.mood.dominance);
        s.push('}');
        match &self.caption {
            CaptionSlot::Hold => {}
            CaptionSlot::Clear => s.push_str(",\"caption\":null"),
            CaptionSlot::Set(c) => {
                s.push_str(",\"caption\":{\"text\":");
                push_str(&mut s, &c.text);
                s.push_str(",\"sources\":[");
                for (i, src) in c.sources.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    push_str(&mut s, src);
                }
                s.push_str("],\"final\":");
                s.push_str(if c.final_ { "true" } else { "false" });
                s.push('}');
            }
        }
        s.push('}');
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Talk,
    Stop,
    End,
    Barge,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Talk => "talk",
            Action::Stop => "stop",
            Action::End => "end",
            Action::Barge => "barge",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "talk" => Some(Action::Talk),
            "stop" => Some(Action::Stop),
            "end" => Some(Action::End),
            "barge" => Some(Action::Barge),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    pub action: Action,
}

impl Control {
    pub fn to_json(self) -> String {
        format!(
            "{{\"schema\":{},\"kind\":\"control\",\"action\":{}}}",
            json_string(SCHEMA),
            json_string(self.action.as_str())
        )
    }
}

/// What `Brain::turn` returns. The real crate is not linked.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOut {
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub propensity: f32,
    pub evidence: Vec<String>,
    pub sources: Vec<String>,
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub mood: Pad,
}

/// Little-endian s16 mono PCM with a 20-byte header.
#[derive(Debug, Clone, PartialEq)]
pub struct PcmFrame {
    pub sample_rate: u32,
    pub seq: u32,
    pub samples: Vec<i16>,
}

impl PcmFrame {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.samples.is_empty() || self.samples.len() > MAX_SAMPLES {
            return Err(ProtocolError::Length);
        }
        if !matches!(self.sample_rate, 8_000 | 16_000 | 24_000 | 48_000) {
            return Err(ProtocolError::Length);
        }
        let mut out = Vec::with_capacity(20 + self.samples.len() * 2);
        out.extend_from_slice(PCM_MAGIC);
        out.push(1); // version
        out.push(1); // s16le
        out.push(1); // mono
        out.push(0);
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&(self.samples.len() as u32).to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        Ok(out)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() < 20 {
            return Err(ProtocolError::Truncated);
        }
        if &buf[0..4] != PCM_MAGIC {
            return Err(ProtocolError::Magic);
        }
        if buf[4] != 1 || buf[5] != 1 || buf[6] != 1 {
            return Err(ProtocolError::Version);
        }
        let sample_rate = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let seq = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let n = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
        if n == 0 || n > MAX_SAMPLES || !matches!(sample_rate, 8_000 | 16_000 | 24_000 | 48_000) {
            return Err(ProtocolError::Length);
        }
        if buf.len() != 20 + n * 2 {
            return Err(ProtocolError::Length);
        }
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let o = 20 + i * 2;
            samples.push(i16::from_le_bytes([buf[o], buf[o + 1]]));
        }
        Ok(PcmFrame {
            sample_rate,
            seq,
            samples,
        })
    }

    /// RMS in 0..=1, full scale at 32768.
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut acc = 0.0f64;
        for s in &self.samples {
            let x = *s as f64 / 32768.0;
            acc += x * x;
        }
        (acc / self.samples.len() as f64).sqrt() as f32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Json,
    Schema,
    State,
    Truncated,
    Magic,
    Version,
    Length,
    TooLarge,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::Json => write!(f, "json"),
            ProtocolError::Schema => write!(f, "schema"),
            ProtocolError::State => write!(f, "state"),
            ProtocolError::Truncated => write!(f, "truncated"),
            ProtocolError::Magic => write!(f, "magic"),
            ProtocolError::Version => write!(f, "version"),
            ProtocolError::Length => write!(f, "length"),
            ProtocolError::TooLarge => write!(f, "too large"),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn decode_server(text: &str) -> Result<ServerEvent, ProtocolError> {
    if text.len() > MAX_JSON {
        return Err(ProtocolError::TooLarge);
    }
    let v = parse_json(text).map_err(|_| ProtocolError::Json)?;
    let obj = v.as_obj().ok_or(ProtocolError::Json)?;
    let schema = obj_str(obj, "schema").ok_or(ProtocolError::Schema)?;
    if schema != SCHEMA {
        return Err(ProtocolError::Schema);
    }
    let kind = match obj_str(obj, "kind") {
        Some("state") => EventKind::State,
        Some("tick") | None => EventKind::Tick,
        Some(_) => return Err(ProtocolError::Json),
    };
    let state = VoiceState::parse(obj_str(obj, "state").ok_or(ProtocolError::State)?)
        .ok_or(ProtocolError::State)?;
    let level = obj_f32(obj, "level").unwrap_or(0.0).clamp(0.0, 1.0);
    let t_ms = obj_u32(obj, "t_ms").ok_or(ProtocolError::Json)?;
    let mood = match obj_get(obj, "mood") {
        Some(Val::Obj(m)) => Pad {
            arousal: obj_f32(m, "arousal").unwrap_or(Pad::RESTING.arousal),
            valence: obj_f32(m, "valence").unwrap_or(Pad::RESTING.valence),
            dominance: obj_f32(m, "dominance").unwrap_or(Pad::RESTING.dominance),
        }
        .sane(),
        _ => Pad::RESTING,
    };
    let caption = match obj_get(obj, "caption") {
        None => CaptionSlot::Hold,
        Some(Val::Null) => CaptionSlot::Clear,
        Some(Val::Obj(c)) => {
            let text = obj_str(c, "text").unwrap_or("").to_string();
            if text.len() > 4_000 {
                return Err(ProtocolError::TooLarge);
            }
            let mut sources = Vec::new();
            if let Some(Val::Arr(items)) = obj_get(c, "sources") {
                for item in items.iter().take(32) {
                    if let Some(s) = item.as_str() {
                        if s.len() > 200 {
                            return Err(ProtocolError::TooLarge);
                        }
                        sources.push(s.to_string());
                    }
                }
            }
            let final_ = match obj_get(c, "final") {
                Some(Val::Bool(b)) => *b,
                _ => true,
            };
            CaptionSlot::Set(Caption {
                text,
                sources,
                final_,
            })
        }
        _ => return Err(ProtocolError::Json),
    };
    Ok(ServerEvent {
        kind,
        state,
        mood,
        level,
        t_ms,
        caption,
    })
}

pub fn decode_control(text: &str) -> Result<Control, ProtocolError> {
    if text.len() > MAX_JSON {
        return Err(ProtocolError::TooLarge);
    }
    let v = parse_json(text).map_err(|_| ProtocolError::Json)?;
    let obj = v.as_obj().ok_or(ProtocolError::Json)?;
    let schema = obj_str(obj, "schema").ok_or(ProtocolError::Schema)?;
    if schema != SCHEMA {
        return Err(ProtocolError::Schema);
    }
    match obj_str(obj, "kind") {
        Some("control") => {}
        _ => return Err(ProtocolError::Json),
    }
    let action = Action::parse(obj_str(obj, "action").ok_or(ProtocolError::Json)?)
        .ok_or(ProtocolError::Json)?;
    Ok(Control { action })
}

fn push_str(out: &mut String, s: &str) {
    out.push_str(&json_string(s));
}

fn push_num(out: &mut String, n: f32) {
    if n.is_finite() {
        out.push_str(&format!("{n}"));
    } else {
        out.push('0');
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Debug)]
enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Val>),
    Obj(Vec<(String, Val)>),
}

impl Val {
    fn as_obj(&self) -> Option<&[(String, Val)]> {
        match self {
            Val::Obj(o) => Some(o),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }
}

fn obj_get<'a>(obj: &'a [(String, Val)], key: &str) -> Option<&'a Val> {
    obj.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn obj_str<'a>(obj: &'a [(String, Val)], key: &str) -> Option<&'a str> {
    obj_get(obj, key).and_then(Val::as_str)
}

fn obj_f32(obj: &[(String, Val)], key: &str) -> Option<f32> {
    match obj_get(obj, key) {
        Some(Val::Num(n)) if n.is_finite() => Some(*n as f32),
        _ => None,
    }
}

fn obj_u32(obj: &[(String, Val)], key: &str) -> Option<u32> {
    match obj_get(obj, key) {
        Some(Val::Num(n))
            if n.is_finite() && *n >= 0.0 && *n <= u32::MAX as f64 && n.fract() == 0.0 =>
        {
            Some(*n as u32)
        }
        _ => None,
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

fn parse_json(text: &str) -> Result<Val, ()> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.skip();
    let v = p.val()?;
    p.skip();
    if p.i != p.s.len() {
        return Err(());
    }
    Ok(v)
}

impl Parser<'_> {
    fn skip(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.i += 1;
        }
    }
    fn val(&mut self) -> Result<Val, ()> {
        self.skip();
        match self.s.get(self.i).copied() {
            Some(b'n') => self.lit(b"null", Val::Null),
            Some(b't') => self.lit(b"true", Val::Bool(true)),
            Some(b'f') => self.lit(b"false", Val::Bool(false)),
            Some(b'"') => Ok(Val::Str(self.string()?)),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err(()),
        }
    }
    fn lit(&mut self, word: &[u8], v: Val) -> Result<Val, ()> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(())
        }
    }
    fn array(&mut self) -> Result<Val, ()> {
        self.i += 1;
        let mut items = Vec::new();
        self.skip();
        if self.s.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Val::Arr(items));
        }
        loop {
            items.push(self.val()?);
            self.skip();
            match self.s.get(self.i).copied() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(()),
            }
        }
        Ok(Val::Arr(items))
    }
    fn object(&mut self) -> Result<Val, ()> {
        self.i += 1;
        let mut items = Vec::new();
        self.skip();
        if self.s.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Val::Obj(items));
        }
        loop {
            self.skip();
            if self.s.get(self.i) != Some(&b'"') {
                return Err(());
            }
            let k = self.string()?;
            self.skip();
            if self.s.get(self.i) != Some(&b':') {
                return Err(());
            }
            self.i += 1;
            items.push((k, self.val()?));
            self.skip();
            match self.s.get(self.i).copied() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(()),
            }
        }
        Ok(Val::Obj(items))
    }
    fn string(&mut self) -> Result<String, ()> {
        self.i += 1;
        let mut out = String::new();
        while let Some(c) = self.s.get(self.i).copied() {
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.s.get(self.i).copied().ok_or(())?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return Err(());
                            }
                            let hex =
                                std::str::from_utf8(&self.s[self.i..self.i + 4]).map_err(|_| ())?;
                            let u = u32::from_str_radix(hex, 16).map_err(|_| ())?;
                            let ch = char::from_u32(u).ok_or(())?;
                            if (0xD800..=0xDFFF).contains(&u) {
                                return Err(());
                            }
                            out.push(ch);
                            self.i += 4;
                        }
                        _ => return Err(()),
                    }
                }
                c if c < 0x20 => return Err(()),
                c => out.push(c as char),
            }
        }
        Err(())
    }
    fn number(&mut self) -> Result<Val, ()> {
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        let digs = self.i;
        while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if self.i == digs {
            return Err(());
        }
        if self.s.get(self.i) == Some(&b'.') {
            self.i += 1;
            let frac = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == frac {
                return Err(());
            }
        }
        if matches!(self.s.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.s.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let exp = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == exp {
                return Err(());
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| ())?;
        let n: f64 = text.parse().map_err(|_| ())?;
        if !n.is_finite() {
            return Err(());
        }
        Ok(Val::Num(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_pad_is_the_horbm_pose() {
        assert_eq!(Pad::RESTING.arousal, 0.3);
        assert_eq!(Pad::RESTING.valence, 0.25);
        assert_eq!(Pad::RESTING.dominance, 0.5);
        let bad = Pad {
            arousal: f32::NAN,
            valence: 4.0,
            dominance: -1.0,
        }
        .sane();
        assert_eq!(bad.arousal, 0.3);
        assert_eq!(bad.valence, 1.0);
        assert_eq!(bad.dominance, 0.0);
    }

    #[test]
    fn server_event_roundtrip_keeps_caption_and_mood() {
        let ev = ServerEvent {
            kind: EventKind::Tick,
            state: VoiceState::Speaking,
            mood: Pad::RESTING,
            level: 0.5,
            t_ms: 640,
            caption: CaptionSlot::Set(Caption {
                text: "Hi. I'm here.".into(),
                sources: vec!["fake:reply-engine".into()],
                final_: true,
            }),
        };
        let back = decode_server(&ev.to_json()).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn caption_null_clears_and_absent_holds() {
        let clear = r#"{"schema":"galion-samantha-dot/1","kind":"state","state":"idle","level":0,"t_ms":0,"mood":{"arousal":0.3,"valence":0.25,"dominance":0.5},"caption":null}"#;
        let ev = decode_server(clear).unwrap();
        assert_eq!(ev.caption, CaptionSlot::Clear);
        assert_eq!(ev.mood, Pad::RESTING);
        let hold = r#"{"schema":"galion-samantha-dot/1","kind":"tick","state":"listening","level":0.25,"t_ms":80,"mood":{"arousal":0.3,"valence":0.25,"dominance":0.5}}"#;
        assert_eq!(decode_server(hold).unwrap().caption, CaptionSlot::Hold);
    }

    #[test]
    fn control_json_parses() {
        for action in [Action::Talk, Action::Stop, Action::End, Action::Barge] {
            let c = Control { action };
            assert_eq!(decode_control(&c.to_json()).unwrap().action, action);
        }
        assert!(decode_control(r#"{"schema":"nope","kind":"control","action":"talk"}"#).is_err());
        assert!(decode_control(
            r#"{"schema":"galion-samantha-dot/1","kind":"control","action":"sing"}"#
        )
        .is_err());
    }

    #[test]
    fn pcm_header_matches_the_known_bytes() {
        let frame = PcmFrame {
            sample_rate: 16_000,
            seq: 1,
            samples: vec![-2],
        };
        let bytes = frame.encode().unwrap();
        assert_eq!(
            bytes,
            [
                0x47, 0x56, 0x50, 0x43, 0x01, 0x01, 0x01, 0x00, 0x80, 0x3e, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xfe, 0xff,
            ]
        );
        assert_eq!(PcmFrame::decode(&bytes).unwrap(), frame);
        assert!(PcmFrame::decode(b"XXXX").is_err());
        assert!(PcmFrame::decode(&[0x47, 0x56, 0x50, 0x43]).is_err());
    }

    #[test]
    fn rms_of_half_scale_is_one_half() {
        let frame = PcmFrame {
            sample_rate: 16_000,
            seq: 0,
            samples: vec![16384; 8],
        };
        let rms = frame.rms();
        assert!((rms - 0.5).abs() < 1e-6, "{rms}");
    }

    #[test]
    fn eighty_ms_frame_is_1280_samples() {
        assert_eq!(PCM_FRAME_SAMPLES, 1_280);
        assert_eq!(TICK_MS, 80);
    }
}
