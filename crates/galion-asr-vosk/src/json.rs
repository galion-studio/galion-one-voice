// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Tiny JSON subset for the Vosk worker protocol. No serde. Objects keep
//! insertion order so tests can match the bytes we emit.

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

fn push_num(out: &mut String, n: f64) {
    if !n.is_finite() {
        out.push('0');
        return;
    }
    if n == n.round() && n.abs() < 1e15 {
        out.push_str(&format!("{}", n as i64));
        return;
    }
    let s = format!("{n:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        out.push('0');
    } else {
        out.push_str(s);
    }
}

fn push_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
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

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }

    fn val(&mut self) -> Result<Json, String> {
        self.skip();
        match self.peek() {
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b't') => self.literal(b"true", Json::Bool(true)),
            Some(b'f') => self.literal(b"false", Json::Bool(false)),
            Some(b'n') => self.literal(b"null", Json::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            Some(c) => Err(format!("unexpected byte {c} in JSON")),
            None => Err("unexpected end of JSON".into()),
        }
    }

    fn literal(&mut self, word: &[u8], v: Json) -> Result<Json, String> {
        for b in word {
            if self.bump() != Some(*b) {
                return Err(format!("expected {}", String::from_utf8_lossy(word)));
            }
        }
        Ok(v)
    }

    fn object(&mut self) -> Result<Json, String> {
        self.bump();
        let mut kv = Vec::new();
        loop {
            self.skip();
            if self.peek() == Some(b'}') {
                self.bump();
                return Ok(Json::Obj(kv));
            }
            if !kv.is_empty() {
                if self.bump() != Some(b',') {
                    return Err("expected comma in object".into());
                }
                self.skip();
            }
            let key = self.string()?;
            self.skip();
            if self.bump() != Some(b':') {
                return Err("expected colon".into());
            }
            kv.push((key, self.val()?));
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip();
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Json::Arr(items));
            }
            if !items.is_empty() {
                if self.bump() != Some(b',') {
                    return Err("expected comma in array".into());
                }
            }
            items.push(self.val()?);
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.skip();
        if self.bump() != Some(b'"') {
            return Err("expected string".into());
        }
        let mut out = String::new();
        loop {
            let c = self
                .bump()
                .ok_or_else(|| "unterminated string".to_string())?;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.bump().ok_or_else(|| "bad escape".to_string())?;
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
                            let mut hex = [0u8; 4];
                            for slot in &mut hex {
                                *slot = self
                                    .bump()
                                    .ok_or_else(|| "short unicode escape".to_string())?;
                            }
                            let s = std::str::from_utf8(&hex).map_err(|_| "bad unicode escape")?;
                            let cp =
                                u32::from_str_radix(s, 16).map_err(|_| "bad unicode escape")?;
                            let ch = char::from_u32(cp).ok_or("bad unicode scalar")?;
                            out.push(ch);
                        }
                        _ => return Err("bad escape".into()),
                    }
                }
                c => self.push_utf8(&mut out, c)?,
            }
        }
    }

    fn push_utf8(&mut self, out: &mut String, first: u8) -> Result<(), String> {
        let width = if first < 0x80 {
            1
        } else if first & 0xE0 == 0xC0 {
            2
        } else if first & 0xF0 == 0xE0 {
            3
        } else if first & 0xF8 == 0xF0 {
            4
        } else {
            return Err("bad utf-8 in string".into());
        };
        let mut buf = [0u8; 4];
        buf[0] = first;
        for i in 1..width {
            buf[i] = self.bump().ok_or_else(|| "truncated utf-8".to_string())?;
        }
        let s = std::str::from_utf8(&buf[..width]).map_err(|_| "bad utf-8 in string")?;
        out.push_str(s);
        Ok(())
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        if self.peek() == Some(b'0') {
            self.i += 1;
        } else if matches!(self.peek(), Some(b'1'..=b'9')) {
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        } else {
            return Err("bad number".into());
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err("bad number".into());
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err("bad number".into());
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "bad number")?;
        let n: f64 = text.parse().map_err(|_| format!("bad number {text}"))?;
        if !n.is_finite() {
            return Err("non-finite number".into());
        }
        Ok(Json::Num(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vosk_result_object_roundtrips_words() {
        let raw = r#"{
          "result" : [{"conf" : 0.980000, "end" : 0.540000, "start" : 0.210000, "word" : "hello"}],
          "text" : "say \"hi\""
        }"#;
        let v = parse(raw).unwrap();
        assert_eq!(v.get("text").and_then(Json::as_str), Some("say \"hi\""));
        let word = &v.get("result").and_then(Json::as_arr).unwrap()[0];
        assert_eq!(word.get("word").and_then(Json::as_str), Some("hello"));
        assert!((word.get("conf").and_then(Json::as_f64).unwrap() - 0.98).abs() < 1e-6);
        let again = parse(&to_line(&v)).unwrap();
        assert_eq!(again.get("text").and_then(Json::as_str), Some("say \"hi\""));
    }
}
