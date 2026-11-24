// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! JSON subset for Vosk result blobs and the galion-asr-vosk/1 lines.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Json, String> {
    let mut p = Parser {
        s: input.as_bytes(),
        i: 0,
    };
    p.skip();
    let v = p.val()?;
    p.skip();
    if p.i != p.s.len() {
        return Err("trailing data after JSON".into());
    }
    Ok(v)
}

pub fn to_line(v: &Json) -> String {
    let mut out = String::new();
    push_json(&mut out, v);
    out
}

pub fn to_pretty(v: &Json) -> String {
    let mut out = String::new();
    pretty(&mut out, v, 0);
    out.push('\n');
    out
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn bump(&mut self) -> Result<u8, String> {
        let b = self
            .peek()
            .ok_or_else(|| "unexpected end of JSON".to_string())?;
        self.i += 1;
        Ok(b)
    }

    fn eat(&mut self, lit: &str) -> Result<(), String> {
        let bytes = lit.as_bytes();
        if self.s.len() - self.i >= bytes.len() && &self.s[self.i..self.i + bytes.len()] == bytes {
            self.i += bytes.len();
            Ok(())
        } else {
            Err(format!("expected {lit}"))
        }
    }

    fn val(&mut self) -> Result<Json, String> {
        self.skip();
        match self.peek() {
            Some(b'n') => self.eat("null").map(|_| Json::Null),
            Some(b't') => self.eat("true").map(|_| Json::Bool(true)),
            Some(b'f') => self.eat("false").map(|_| Json::Bool(false)),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => self.arr(),
            Some(b'{') => self.obj(),
            Some(b'-') | Some(b'0'..=b'9') => self.num(),
            Some(other) => Err(format!("unexpected JSON byte {other}")),
            None => Err("unexpected end of JSON".into()),
        }
    }

    fn num(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
        ) {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "bad number")?;
        let n: f64 = s.parse().map_err(|_| format!("bad number {s}"))?;
        if !n.is_finite() {
            return Err("non-finite number".into());
        }
        Ok(Json::Num(n))
    }

    fn string(&mut self) -> Result<String, String> {
        self.bump()?;
        let mut buf = Vec::new();
        loop {
            let b = self.bump()?;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self.bump()?;
                    match e {
                        b'"' | b'\\' | b'/' => buf.push(e),
                        b'b' => buf.push(0x08),
                        b'f' => buf.push(0x0c),
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'u' => {
                            let cp = self.hex4()?;
                            let ch =
                                char::from_u32(cp).ok_or_else(|| format!("bad codepoint {cp}"))?;
                            let mut tmp = [0u8; 4];
                            buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                        }
                        _ => return Err(format!("bad escape {e}")),
                    }
                }
                c if c < 0x20 => return Err("control character in string".into()),
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).map_err(|_| "string is not utf-8".into())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut n = 0u32;
        for _ in 0..4 {
            let b = self.bump()?;
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err("bad hex escape".into()),
            };
            n = (n << 4) | u32::from(d);
        }
        Ok(n)
    }

    fn arr(&mut self) -> Result<Json, String> {
        self.bump()?;
        let mut items = Vec::new();
        self.skip();
        if self.peek() == Some(b']') {
            self.bump()?;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.val()?);
            self.skip();
            match self.bump()? {
                b']' => break,
                b',' => continue,
                _ => return Err("expected ',' or ']'".into()),
            }
        }
        Ok(Json::Arr(items))
    }

    fn obj(&mut self) -> Result<Json, String> {
        self.bump()?;
        let mut kv = Vec::new();
        self.skip();
        if self.peek() == Some(b'}') {
            self.bump()?;
            return Ok(Json::Obj(kv));
        }
        loop {
            self.skip();
            if self.peek() != Some(b'"') {
                return Err("object key must be a string".into());
            }
            let key = self.string()?;
            self.skip();
            if self.bump()? != b':' {
                return Err("expected ':'".into());
            }
            let val = self.val()?;
            kv.push((key, val));
            self.skip();
            match self.bump()? {
                b'}' => break,
                b',' => continue,
                _ => return Err("expected ',' or '}'".into()),
            }
        }
        Ok(Json::Obj(kv))
    }
}

fn push_json(out: &mut String, v: &Json) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Num(n) => push_num(out, *n),
        Json::Str(s) => push_str(out, s),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_json(out, item);
            }
            out.push(']');
        }
        Json::Obj(kv) => {
            out.push('{');
            for (i, (k, val)) in kv.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_str(out, k);
                out.push(':');
                push_json(out, val);
            }
            out.push('}');
        }
    }
}

fn pretty(out: &mut String, v: &Json, indent: usize) {
    match v {
        Json::Arr(items) if items.is_empty() => out.push_str("[]"),
        Json::Obj(kv) if kv.is_empty() => out.push_str("{}"),
        Json::Arr(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                pad(out, indent + 2);
                pretty(out, item, indent + 2);
                if i + 1 != items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            pad(out, indent);
            out.push(']');
        }
        Json::Obj(kv) => {
            out.push_str("{\n");
            for (i, (k, val)) in kv.iter().enumerate() {
                pad(out, indent + 2);
                push_str(out, k);
                out.push_str(": ");
                pretty(out, val, indent + 2);
                if i + 1 != kv.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            pad(out, indent);
            out.push('}');
        }
        other => push_json(out, other),
    }
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

fn push_num(out: &mut String, n: f64) {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        out.push_str(&format!("{}", n as i64));
    } else {
        let s = format!("{n:.8}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        out.push_str(s);
    }
}

fn push_str(out: &mut String, s: &str) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vosk_partial_and_final() {
        let partial = parse("{\n  \"partial\" : \"hello world\"\n}").unwrap();
        assert_eq!(
            partial.get("partial").and_then(Json::as_str),
            Some("hello world")
        );
        let blob = r#"{"result":[{"conf":0.87,"end":0.45,"start":0.12,"word":"hello"},{"conf":1,"end":0.9,"start":0.45,"word":"[unk]"}],"text":"hello [unk]"}"#;
        let v = parse(blob).unwrap();
        assert_eq!(v.get("text").and_then(Json::as_str), Some("hello [unk]"));
        let words = v.get("result").and_then(Json::as_arr).unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].get("word").and_then(Json::as_str), Some("hello"));
        assert_eq!(words[0].get("conf").and_then(Json::as_f64), Some(0.87));
    }

    #[test]
    fn escapes_roundtrip() {
        let v = Json::Obj(vec![("t".into(), Json::Str("say \"hi\"\n".into()))]);
        let line = to_line(&v);
        let back = parse(&line).unwrap();
        assert_eq!(back.get("t").and_then(Json::as_str), Some("say \"hi\"\n"));
    }
}
