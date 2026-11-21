// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `galion-samantha-ws/1`, the socket in swarm 16 architecture (#220).
//!
//! Text frames are JSON. Binary frames are an 8-byte `GS` header plus
//! little-endian s16 mono PCM. HORBM is not in this crate.
//! [`crate::engine::ReplyEngine`] is the seam a later `Brain::turn` can fill.

use std::fmt;

/// Schema on every JSON frame. Same string as `voice-stack/contract`.
pub const SCHEMA: &str = "galion-samantha-ws/1";

/// Loopback port from the architecture. Not 7878, 7879, 7880, 8642, or 8766.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7881";

/// WebSocket path. The page itself is `GET /`.
pub const WS_PATH: &str = "/v1/samantha";

/// Mood clock. Same period as `galion-heartbeat` `FRAME_MS`.
pub const TICK_MS: u32 = 80;

/// Page capture rate. The header marks it as 16 kHz. The speaker path is 24 kHz.
pub const PCM_RATE_HZ: u32 = 16_000;

/// Samples in one 80 ms frame at [`PCM_RATE_HZ`].
pub const PCM_FRAME_SAMPLES: usize = 1_280;

const MAX_SAMPLES: usize = 48_000;
const MAX_JSON: usize = 65_536;

/// Pleasure / arousal / dominance. [`Pad::RESTING`] is the HORBM rest pose.
/// Valence is about -1..=1. Arousal and dominance are clamped to 0..=1.
/// Non-finite channels fall back to rest.
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

/// Local motion includes thinking. The wire `dot` frame does not.
/// `wire` maps thinking back to idle so the socket stays inside the architecture.
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

    /// `dot` frames from the architecture: idle, listening, speaking.
    pub fn wire(self) -> &'static str {
        match self {
            VoiceState::Listening => "listening",
            VoiceState::Speaking => "speaking",
            VoiceState::Idle | VoiceState::Thinking => "idle",
        }
    }

    pub fn parse_wire(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(VoiceState::Idle),
            "listening" => Some(VoiceState::Listening),
            "speaking" => Some(VoiceState::Speaking),
            _ => None,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDir {
    Mic = 0,
    Speaker = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateCode {
    Hz24k = 0,
    Hz16k = 1,
}

impl RateCode {
    pub fn hz(self) -> u32 {
        match self {
            RateCode::Hz24k => 24_000,
            RateCode::Hz16k => 16_000,
        }
    }
}

/// Eight-byte `GS` header, then s16le mono. Same layout as `contract::ws`.
#[derive(Debug, Clone, PartialEq)]
pub struct PcmFrame {
    pub dir: AudioDir,
    pub rate: RateCode,
    pub seq: u16,
    pub samples: Vec<i16>,
}

impl PcmFrame {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.samples.is_empty() || self.samples.len() > MAX_SAMPLES {
            return Err(ProtocolError::Length);
        }
        let mut out = Vec::with_capacity(8 + self.samples.len() * 2);
        out.extend_from_slice(&[b'G', b'S', 1, self.dir as u8, self.rate as u8, 0]);
        out.extend_from_slice(&self.seq.to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        Ok(out)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() < 8 {
            return Err(ProtocolError::Truncated);
        }
        if buf[0] != b'G' || buf[1] != b'S' {
            return Err(ProtocolError::Magic);
        }
        if buf[2] != 1 {
            return Err(ProtocolError::Version);
        }
        let dir = match buf[3] {
            0 => AudioDir::Mic,
            1 => AudioDir::Speaker,
            _ => return Err(ProtocolError::Length),
        };
        let rate = match buf[4] {
            0 => RateCode::Hz24k,
            1 => RateCode::Hz16k,
            _ => return Err(ProtocolError::Length),
        };
        if buf.len() % 2 != 0 {
            return Err(ProtocolError::Length);
        }
        let seq = u16::from_le_bytes([buf[6], buf[7]]);
        let mut samples = Vec::with_capacity((buf.len() - 8) / 2);
        for chunk in buf[8..].chunks_exact(2) {
            samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        if samples.is_empty() || samples.len() > MAX_SAMPLES {
            return Err(ProtocolError::Length);
        }
        Ok(PcmFrame {
            dir,
            rate,
            seq,
            samples,
        })
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoodSource {
    Brain,
    Resting,
}

impl MoodSource {
    pub fn as_str(self) -> &'static str {
        match self {
            MoodSource::Brain => "brain",
            MoodSource::Resting => "resting",
        }
    }
}

/// One text frame from the server. The page paints these.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerMsg {
    Welcome,
    Mood {
        t_ms: u32,
        source: MoodSource,
        pad: Pad,
    },
    Dot {
        state: VoiceState,
        level: f32,
    },
    Turn {
        reply: String,
        sources: Vec<String>,
        pad: Pad,
    },
    Pong,
    Error {
        error: String,
    },
}

impl ServerMsg {
    pub fn to_json(&self) -> String {
        match self {
            ServerMsg::Welcome => format!(
                "{{\"schema\":{schema},\"type\":\"welcome\",\"frame_ms\":{TICK_MS},\"sample_rate\":24000,\"dot\":\"idle\"}}",
                schema = json_string(SCHEMA)
            ),
            ServerMsg::Mood { t_ms, source, pad } => format!(
                "{{\"schema\":{schema},\"type\":\"mood\",\"t_ms\":{t_ms},\"source\":{source},\"pad\":{pad}}}",
                schema = json_string(SCHEMA),
                source = json_string(source.as_str()),
                pad = pad_json(*pad)
            ),
            ServerMsg::Dot { state, level } => format!(
                "{{\"schema\":{schema},\"type\":\"dot\",\"state\":{state},\"level\":{level}}}",
                schema = json_string(SCHEMA),
                state = json_string(state.wire()),
                level = num(*level)
            ),
            ServerMsg::Turn { reply, sources, pad } => {
                let mut s = format!(
                    "{{\"schema\":{schema},\"type\":\"turn\",\"reply\":{reply},\"intent\":null,\"arm\":null,\"propensity\":0,\"evidence\":[],\"sources\":[",
                    schema = json_string(SCHEMA),
                    reply = json_string(reply)
                );
                for (i, src) in sources.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    s.push_str(&json_string(src));
                }
                s.push_str("],\"violations\":0,\"lookup_us\":0,\"took_us\":0,\"mood\":");
                s.push_str(&pad_json(*pad));
                s.push('}');
                s
            }
            ServerMsg::Pong => format!(
                "{{\"schema\":{schema},\"type\":\"pong\"}}",
                schema = json_string(SCHEMA)
            ),
            ServerMsg::Error { error } => format!(
                "{{\"schema\":{schema},\"type\":\"error\",\"error\":{error}}}",
                schema = json_string(SCHEMA),
                error = json_string(error)
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMsg {
    Hello,
    Stop,
    Ping,
    Text(String),
}

pub fn hello_json() -> String {
    format!(
        "{{\"schema\":{schema},\"type\":\"hello\",\"session\":null}}",
        schema = json_string(SCHEMA)
    )
}

pub fn stop_json() -> String {
    format!(
        "{{\"schema\":{schema},\"type\":\"stop\"}}",
        schema = json_string(SCHEMA)
    )
}

pub fn text_json(text: &str) -> String {
    format!(
        "{{\"schema\":{schema},\"type\":\"text\",\"text\":{text}}}",
        schema = json_string(SCHEMA),
        text = json_string(text)
    )
}

fn pad_json(pad: Pad) -> String {
    format!(
        "{{\"arousal\":{},\"valence\":{},\"dominance\":{}}}",
        num(pad.arousal),
        num(pad.valence),
        num(pad.dominance)
    )
}

fn num(n: f32) -> String {
    if n.is_finite() {
        format!("{n}")
    } else {
        "0".to_string()
    }
}

pub fn decode_server(text: &str) -> Result<ServerMsg, ProtocolError> {
    if text.len() > MAX_JSON {
        return Err(ProtocolError::TooLarge);
    }
    let v = parse_json(text).map_err(|_| ProtocolError::Json)?;
    let obj = v.as_obj().ok_or(ProtocolError::Json)?;
    let schema = obj_str(obj, "schema").ok_or(ProtocolError::Schema)?;
    if schema != SCHEMA {
        return Err(ProtocolError::Schema);
    }
    match obj_str(obj, "type").ok_or(ProtocolError::Json)? {
        "welcome" => Ok(ServerMsg::Welcome),
        "pong" => Ok(ServerMsg::Pong),
        "error" => Ok(ServerMsg::Error {
            error: obj_str(obj, "error").unwrap_or("error").to_string(),
        }),
        "dot" => {
            let state = VoiceState::parse_wire(obj_str(obj, "state").unwrap_or("idle"))
                .ok_or(ProtocolError::State)?;
            let level = obj_f32(obj, "level").unwrap_or(0.0).clamp(0.0, 1.0);
            Ok(ServerMsg::Dot { state, level })
        }
        "mood" => {
            let t_ms = obj_u32(obj, "t_ms").unwrap_or(0);
            let source = match obj_str(obj, "source") {
                Some("brain") => MoodSource::Brain,
                _ => MoodSource::Resting,
            };
            let pad = match obj_get(obj, "pad") {
                Some(Val::Obj(m)) => read_pad(m),
                _ => Pad::RESTING,
            };
            Ok(ServerMsg::Mood { t_ms, source, pad })
        }
        "turn" => {
            let reply = obj_str(obj, "reply").unwrap_or("").to_string();
            if reply.len() > 4_000 {
                return Err(ProtocolError::TooLarge);
            }
            let sources = read_sources(obj)?;
            let pad = match obj_get(obj, "mood") {
                Some(Val::Obj(m)) => read_pad(m),
                _ => Pad::RESTING,
            };
            Ok(ServerMsg::Turn {
                reply,
                sources,
                pad,
            })
        }
        "vad" => Ok(ServerMsg::Dot {
            state: if obj_str(obj, "event") == Some("start") {
                VoiceState::Listening
            } else {
                VoiceState::Idle
            },
            level: 0.0,
        }),
        _ => Err(ProtocolError::Json),
    }
}

fn read_pad(m: &[(String, Val)]) -> Pad {
    Pad {
        arousal: obj_f32(m, "arousal").unwrap_or(Pad::RESTING.arousal),
        valence: obj_f32(m, "valence").unwrap_or(Pad::RESTING.valence),
        dominance: obj_f32(m, "dominance").unwrap_or(Pad::RESTING.dominance),
    }
    .sane()
}

fn read_sources(obj: &[(String, Val)]) -> Result<Vec<String>, ProtocolError> {
    let mut sources = Vec::new();
    if let Some(Val::Arr(items)) = obj_get(obj, "sources") {
        for item in items.iter().take(32) {
            if let Some(s) = item.as_str() {
                if s.len() > 200 {
                    return Err(ProtocolError::TooLarge);
                }
                sources.push(s.to_string());
            }
        }
    }
    Ok(sources)
}

pub fn decode_client(text: &str) -> Result<ClientMsg, ProtocolError> {
    if text.len() > MAX_JSON {
        return Err(ProtocolError::TooLarge);
    }
    let v = parse_json(text).map_err(|_| ProtocolError::Json)?;
    let obj = v.as_obj().ok_or(ProtocolError::Json)?;
    let schema = obj_str(obj, "schema").ok_or(ProtocolError::Schema)?;
    if schema != SCHEMA {
        return Err(ProtocolError::Schema);
    }
    match obj_str(obj, "type").ok_or(ProtocolError::Json)? {
        "hello" => Ok(ClientMsg::Hello),
        "stop" => Ok(ClientMsg::Stop),
        "ping" => Ok(ClientMsg::Ping),
        "text" => Ok(ClientMsg::Text(
            obj_str(obj, "text").unwrap_or("").to_string(),
        )),
        _ => Err(ProtocolError::Json),
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
#[allow(dead_code)]
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
    fn welcome_mood_and_turn_roundtrip() {
        let welcome = decode_server(&ServerMsg::Welcome.to_json()).unwrap();
        assert_eq!(welcome, ServerMsg::Welcome);
        let mood = ServerMsg::Mood {
            t_ms: 80,
            source: MoodSource::Resting,
            pad: Pad::RESTING,
        };
        assert_eq!(decode_server(&mood.to_json()).unwrap(), mood);
        let turn = ServerMsg::Turn {
            reply: "Hi. I'm here.".into(),
            sources: vec!["fake:reply-engine".into()],
            pad: Pad::RESTING,
        };
        assert_eq!(decode_server(&turn.to_json()).unwrap(), turn);
        assert!(decode_server(r#"{"schema":"nope","type":"pong"}"#).is_err());
    }

    #[test]
    fn client_frames_are_hello_stop_and_text() {
        assert_eq!(decode_client(&hello_json()).unwrap(), ClientMsg::Hello);
        assert_eq!(decode_client(&stop_json()).unwrap(), ClientMsg::Stop);
        assert_eq!(
            decode_client(&text_json("")).unwrap(),
            ClientMsg::Text(String::new())
        );
        assert!(decode_client(r#"{"schema":"galion-samantha-ws/1","type":"sing"}"#).is_err());
    }

    #[test]
    fn pcm_header_is_the_gs_layout() {
        let frame = PcmFrame {
            dir: AudioDir::Mic,
            rate: RateCode::Hz16k,
            seq: 1,
            samples: vec![-2],
        };
        let bytes = frame.encode().unwrap();
        assert_eq!(bytes, [b'G', b'S', 1, 0, 1, 0, 1, 0, 0xfe, 0xff]);
        assert_eq!(PcmFrame::decode(&bytes).unwrap(), frame);
        assert!(PcmFrame::decode(b"XXXX").is_err());
        let speaker = PcmFrame {
            dir: AudioDir::Speaker,
            rate: RateCode::Hz24k,
            seq: 7,
            samples: vec![0, 1, -1, 32767],
        };
        let back = PcmFrame::decode(&speaker.encode().unwrap()).unwrap();
        assert_eq!(back.rate.hz(), 24_000);
        assert_eq!(back.seq, 7);
        assert_eq!(back.samples, speaker.samples);
    }

    #[test]
    fn rms_of_half_scale_is_one_half() {
        let frame = PcmFrame {
            dir: AudioDir::Mic,
            rate: RateCode::Hz16k,
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
        assert_eq!(SCHEMA, "galion-samantha-ws/1");
        assert_eq!(DEFAULT_ADDR, "127.0.0.1:7881");
        assert_eq!(WS_PATH, "/v1/samantha");
    }
}
