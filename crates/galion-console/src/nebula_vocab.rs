//! Nebula vocabulary for the emotion halo.
//!
//! The import the halo uses is
//! `crates/galion-console/assets/nebula/vocabulary.mjs`.
//! This module reads that file so the same table can be checked from
//! `cargo test`. It does not serve a page. The one-dot screen imports the
//! module; that wiring is checked in the tests below.
//!
//! A missing reading, `sense: false`, the label `neutral`, and confidence
//! below [`CONFIDENCE_FLOOR`] resolve to an empty look. Nothing is invented
//! in their place.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Same floor as `EmotionFrame::label`: below this, the frame is unknown.
pub const CONFIDENCE_FLOOR: f64 = 0.2;

const SOURCE: &str = include_str!("../assets/nebula/vocabulary.mjs");

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub present: bool,
    pub id: Option<String>,
    pub reason: &'static str,
    pub amber: f64,
    pub amber_edge: f64,
    pub alpha: f64,
    pub density: f64,
    pub ratio: Option<String>,
    pub depth: f64,
    pub drift_turns: f64,
    pub period_s: f64,
    pub drift_hz: f64,
}

#[derive(Clone, Debug)]
pub struct State {
    pub id: String,
    pub present: bool,
    pub sources: Vec<String>,
    pub aliases: Vec<String>,
    pub amber: f64,
    pub amber_edge: f64,
    pub alpha: f64,
    pub density: f64,
    pub ratio: String,
    pub depth: f64,
    pub drift_turns: f64,
}

#[derive(Clone, Debug)]
pub struct Vocabulary {
    pub schema: String,
    pub site: String,
    pub ground: String,
    pub dot: String,
    pub amber: String,
    pub violet: String,
    pub amber_rgb: [u8; 3],
    pub violet_rgb: [u8; 3],
    pub reference_hz: f64,
    pub octaves_down: u32,
    pub confidence_floor: f64,
    pub states: Vec<State>,
    index: HashMap<String, usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct Reading<'a> {
    pub sense: Option<bool>,
    pub label: Option<&'a str>,
    pub confidence: Option<f64>,
    pub arousal: Option<f64>,
}

impl Vocabulary {
    pub fn base_period_s(&self) -> f64 {
        2f64.powi(self.octaves_down as i32) / self.reference_hz
    }

    pub fn pulse_period_s(&self, ratio: &str) -> f64 {
        let (num, den) = match ratio {
            "2:1" => (2.0, 1.0),
            "3:2" => (3.0, 2.0),
            "5:4" => (5.0, 4.0),
            _ => return 0.0,
        };
        self.base_period_s() * (num / den)
    }

    pub fn state(&self, id: &str) -> Option<&State> {
        self.index.get(id).map(|i| &self.states[*i])
    }
}

pub fn vocabulary() -> &'static Vocabulary {
    static CELL: OnceLock<Vocabulary> = OnceLock::new();
    CELL.get_or_init(|| parse_vocabulary(table_json()).expect("nebula vocabulary JSON"))
}

/// `None` is a missing reading: no cloud.
pub fn lookup(reading: Option<Reading<'_>>) -> Look {
    let Some(reading) = reading else {
        return empty("no-reading");
    };
    if reading.sense == Some(false) {
        return empty("sense-off");
    }
    if let Some(confidence) = reading.confidence {
        if !confidence.is_finite() || confidence < CONFIDENCE_FLOOR {
            return empty("low-confidence");
        }
    }
    let label = normalise(reading.label.unwrap_or(""));
    if label.is_empty() || label == "neutral" || label == "unknown" || label == "none" {
        return empty("neutral");
    }
    let vocab = vocabulary();
    let Some(row) = vocab.state(&label) else {
        return empty("unknown-label");
    };
    if !row.present {
        return empty("neutral");
    }
    let gain = match reading.arousal {
        Some(a) if a.is_finite() => 0.82 + 0.18 * a.clamp(0.0, 1.0),
        _ => 1.0,
    };
    let period_s = vocab.pulse_period_s(&row.ratio);
    Look {
        present: true,
        id: Some(row.id.clone()),
        reason: "",
        amber: row.amber,
        amber_edge: row.amber_edge,
        alpha: row.alpha * gain,
        density: row.density * gain,
        ratio: Some(row.ratio.clone()),
        depth: row.depth * gain,
        drift_turns: row.drift_turns,
        period_s,
        drift_hz: if period_s > 0.0 {
            row.drift_turns / period_s
        } else {
            0.0
        },
    }
}

fn empty(reason: &'static str) -> Look {
    Look {
        present: false,
        id: None,
        reason,
        amber: 0.0,
        amber_edge: 0.0,
        alpha: 0.0,
        density: 0.0,
        ratio: None,
        depth: 0.0,
        drift_turns: 0.0,
        period_s: 0.0,
        drift_hz: 0.0,
    }
}

fn normalise(label: &str) -> String {
    label
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || *c == '-')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn table_json() -> &'static str {
    let marker = "export const TABLE = ";
    let start = SOURCE.find(marker).expect("TABLE export") + marker.len();
    let bytes = SOURCE.as_bytes();
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let json_start = i;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    &SOURCE[json_start..i]
}

fn parse_vocabulary(json: &str) -> Result<Vocabulary, String> {
    let value = Parser::new(json).parse()?;
    let schema = value.str("schema")?.to_string();
    let brand = value.key("brand")?;
    let clock = value.key("clock")?;
    let mut states = Vec::new();
    let mut index = HashMap::new();
    for row in value.key("states")?.arr()? {
        let state = State {
            id: row.str("id")?.to_string(),
            present: row.bool("present")?,
            sources: strings(row.key("sources")?)?,
            aliases: strings(row.key("aliases")?)?,
            amber: row.num("amber")?,
            amber_edge: row.num("amberEdge")?,
            alpha: row.num("alpha")?,
            density: row.num("density")?,
            ratio: row.str("ratio")?.to_string(),
            depth: row.num("depth")?,
            drift_turns: row.num("driftTurns")?,
        };
        let at = states.len();
        index.insert(state.id.clone(), at);
        for alias in &state.aliases {
            index.insert(alias.clone(), at);
        }
        states.push(state);
    }
    let amber_rgb = rgb(brand.key("amberRgb")?)?;
    let violet_rgb = rgb(brand.key("violetRgb")?)?;
    Ok(Vocabulary {
        schema,
        site: brand.str("site")?.to_string(),
        ground: brand.str("ground")?.to_string(),
        dot: brand.str("dot")?.to_string(),
        amber: brand.str("amber")?.to_string(),
        violet: brand.str("violet")?.to_string(),
        amber_rgb,
        violet_rgb,
        reference_hz: clock.num("referenceHz")?,
        octaves_down: clock.num("octavesDown")? as u32,
        confidence_floor: value.num("confidenceFloor")?,
        states,
        index,
    })
}

fn strings(v: &Value) -> Result<Vec<String>, String> {
    v.arr()?
        .iter()
        .map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn rgb(v: &Value) -> Result<[u8; 3], String> {
    let xs = v.arr()?;
    if xs.len() != 3 {
        return Err("rgb".into());
    }
    Ok([
        xs[0].as_num()? as u8,
        xs[1].as_num()? as u8,
        xs[2].as_num()? as u8,
    ])
}

#[derive(Clone, Debug)]
enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    fn key(&self, name: &str) -> Result<&Value, String> {
        match self {
            Value::Obj(pairs) => pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .ok_or_else(|| format!("missing {name}")),
            _ => Err(format!("{name} is not an object field")),
        }
    }
    fn str(&self, name: &str) -> Result<&str, String> {
        self.key(name)?.as_str()
    }
    fn num(&self, name: &str) -> Result<f64, String> {
        self.key(name)?.as_num()
    }
    fn bool(&self, name: &str) -> Result<bool, String> {
        self.key(name)?.as_bool()
    }
    fn as_str(&self) -> Result<&str, String> {
        match self {
            Value::Str(s) => Ok(s),
            _ => Err("string".into()),
        }
    }
    fn as_num(&self) -> Result<f64, String> {
        match self {
            Value::Num(n) => Ok(*n),
            _ => Err("number".into()),
        }
    }
    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err("bool".into()),
        }
    }
    fn arr(&self) -> Result<&[Value], String> {
        match self {
            Value::Arr(xs) => Ok(xs),
            _ => Err("array".into()),
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            i: 0,
        }
    }
    fn parse(mut self) -> Result<Value, String> {
        let v = self.value()?;
        self.skip();
        if self.i != self.s.len() {
            return Err("trailing json".into());
        }
        Ok(v)
    }
    fn skip(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.skip();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        self.skip();
        match self.s.get(self.i).copied() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.lit(b"true", Value::Bool(true)),
            Some(b'f') => self.lit(b"false", Value::Bool(false)),
            Some(b'n') => self.lit(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err("value".into()),
        }
    }
    fn lit(&mut self, word: &[u8], v: Value) -> Result<Value, String> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err("literal".into())
        }
    }
    fn object(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut pairs = Vec::new();
        if self.eat(b'}') {
            return Ok(Value::Obj(pairs));
        }
        loop {
            self.skip();
            let key = self.string()?;
            if !self.eat(b':') {
                return Err("colon".into());
            }
            pairs.push((key, self.value()?));
            if self.eat(b'}') {
                return Ok(Value::Obj(pairs));
            }
            if !self.eat(b',') {
                return Err("comma".into());
            }
        }
    }
    fn array(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut xs = Vec::new();
        if self.eat(b']') {
            return Ok(Value::Arr(xs));
        }
        loop {
            xs.push(self.value()?);
            if self.eat(b']') {
                return Ok(Value::Arr(xs));
            }
            if !self.eat(b',') {
                return Err("comma".into());
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.skip();
        if !self.eat(b'"') {
            return Err("quote".into());
        }
        let mut out = String::new();
        while let Some(c) = self.s.get(self.i).copied() {
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.s.get(self.i).copied().ok_or("escape")?;
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e as char),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        _ => return Err("escape".into()),
                    }
                }
                c => out.push(c as char),
            }
        }
        Err("string".into())
    }
    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if self.s.get(self.i) == Some(&b'.') {
            self.i += 1;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "num".to_string())?;
        let n = text.parse::<f64>().map_err(|_| "num".to_string())?;
        Ok(Value::Num(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn read(label: &str) -> Reading<'_> {
        Reading {
            sense: Some(true),
            label: Some(label),
            confidence: Some(1.0),
            arousal: None,
        }
    }

    #[test]
    fn clock_brand_and_floor_match_the_shipped_module() {
        let v = vocabulary();
        assert_eq!(v.schema, "galion-nebula-vocabulary/1");
        assert_eq!(v.site, "galion.app");
        assert_eq!(v.ground, "#0A0A0C");
        assert_eq!(v.dot, "#F5F5F7");
        assert_eq!(v.amber, "#F5A524");
        assert_eq!(v.violet, "#7C3AED");
        assert_eq!(v.amber_rgb, [245, 165, 36]);
        assert_eq!(v.violet_rgb, [124, 58, 237]);
        assert_eq!(v.reference_hz, 432.0);
        assert_eq!(v.octaves_down, 10);
        near(v.base_period_s(), 1024.0 / 432.0);
        near(v.pulse_period_s("2:1"), (1024.0 / 432.0) * 2.0);
        near(v.pulse_period_s("3:2"), (1024.0 / 432.0) * 1.5);
        near(v.pulse_period_s("5:4"), (1024.0 / 432.0) * 1.25);
        assert_eq!(v.pulse_period_s("9:8"), 0.0);
        near(v.confidence_floor, CONFIDENCE_FLOOR);
        assert_eq!(v.states.len(), 16);
    }

    #[test]
    fn feed_states_stay_on_the_gradient_and_faint() {
        let v = vocabulary();
        let affect = [
            "neutral", "calm", "joy", "excited", "sad", "tired", "angry", "anxious",
        ];
        let dot = ["bright", "warm", "tense", "low", "energised", "calm"];
        let mood = ["grief", "joy", "vulnerability", "quiet", "calm"];
        for label in affect.into_iter().chain(dot).chain(mood) {
            assert!(v.state(label).is_some(), "{label}");
        }
        for row in &v.states {
            assert!(
                row.alpha <= 0.1 && row.density <= 0.09 && row.depth <= 0.14,
                "{}",
                row.id
            );
            assert!(row.drift_turns <= 0.4, "{}", row.id);
            assert!((0.0..=1.0).contains(&row.amber) && (0.0..=1.0).contains(&row.amber_edge));
            assert!(matches!(row.ratio.as_str(), "2:1" | "3:2" | "5:4"));
            if row.present {
                let n = (row.density * 40.0).round() as i32;
                assert!((1..=3).contains(&n), "{}", row.id);
            }
        }
        assert!(!v.state("neutral").unwrap().present);
        let calm = v.state("calm").unwrap();
        assert!(calm.sources.iter().any(|s| s == "affect"));
        assert!(calm.sources.iter().any(|s| s == "dot"));
        assert!(calm.sources.iter().any(|s| s == "mood"));
        near(calm.amber, 0.46);
        near(v.state("joy").unwrap().amber, 0.84);
        near(v.state("grief").unwrap().amber, 0.08);
        near(v.state("quiet").unwrap().amber, 0.28);
        near(v.state("vulnerability").unwrap().amber, 0.22);
    }

    #[test]
    fn absent_readings_are_empty_and_anger_does_not_outrun_joy() {
        assert_eq!(lookup(None).reason, "no-reading");
        assert!(!lookup(None).present);
        let off = lookup(Some(Reading {
            sense: Some(false),
            label: Some("joy"),
            confidence: Some(1.0),
            arousal: Some(0.8),
        }));
        assert_eq!(off.reason, "sense-off");
        assert_eq!(off.alpha, 0.0);
        let neutral = lookup(Some(read("neutral")));
        assert_eq!(neutral.reason, "neutral");
        assert!(
            lookup(Some(Reading {
                sense: Some(true),
                label: Some("joy"),
                confidence: Some(0.199),
                arousal: None,
            }))
            .reason
                == "low-confidence"
        );
        let edge = lookup(Some(Reading {
            sense: Some(true),
            label: Some("joy"),
            confidence: Some(0.2),
            arousal: None,
        }));
        assert!(edge.present && edge.id.as_deref() == Some("joy"));
        assert_eq!(lookup(Some(read("curious"))).reason, "unknown-label");
        assert_eq!(lookup(Some(read("seductive"))).reason, "unknown-label");
        assert_eq!(lookup(Some(read("happy"))).id.as_deref(), Some("joy"));
        let joy = lookup(Some(read("joy")));
        let calm = lookup(Some(read("calm")));
        let angry = lookup(Some(read("angry")));
        assert!(angry.drift_hz < calm.drift_hz && calm.drift_hz < joy.drift_hz);
        assert!(angry.density < calm.density && calm.density < joy.density);
        assert_eq!(angry.ratio.as_deref(), Some("2:1"));
        let quiet = lookup(Some(Reading {
            sense: Some(true),
            label: Some("joy"),
            confidence: Some(1.0),
            arousal: Some(0.0),
        }));
        near(quiet.density, 0.07 * 0.82);
    }

    /// #121 left the one-dot page unwired so the vocabulary could land on its
    /// own. This branch is that wiring: the one-dot page is the halo surface.
    /// `drawNebula` paints the wash only. A missing reading, neutral, low
    /// confidence, and `sense: false` still draw nothing, and the preview
    /// sheet stays free of audio.
    #[test]
    fn the_one_dot_page_imports_the_halo() {
        let page = include_str!("../assets/v1-screen.html");
        let preview = include_str!("../assets/nebula/preview.html");
        assert!(page.contains("import { drawNebula } from \"/nebula/vocabulary.mjs\""));
        assert!(page.contains("drawNebula(cx,input,tms/1000,"));
        assert!(page.contains("estimate===\"none\""));
        assert!(!page.contains("return {neutral:1}"));
        assert!(preview.contains("from \"./vocabulary.mjs\""));
        assert!(preview.contains("galion.app"));
        assert!(preview.contains("no cloud"));
        assert!(!preview.contains("<audio"));
        assert!(!SOURCE.contains("speechSynthesis"));
        assert!(!SOURCE.contains("AudioContext"));
        let status = std::process::Command::new("node")
            .arg("--test")
            .arg("vocabulary.test.mjs")
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/nebula"))
            .status()
            .expect("node --test");
        assert!(status.success(), "vocabulary.test.mjs");
    }
}
