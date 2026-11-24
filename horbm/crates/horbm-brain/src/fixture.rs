// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The committed clause fixture. Parsed with std only. No serde.

use crate::retrieve::StoredClause;

const FIXTURE_JSON: &str = include_str!("../fixtures/clauses.json");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    pub miss_id: String,
    pub miss_text: String,
    pub clauses: Vec<StoredClause>,
}

impl Fixture {
    pub fn committed() -> Self {
        Self::parse(FIXTURE_JSON).expect("committed clause fixture")
    }

    pub fn parse(json: &str) -> Result<Self, String> {
        let value = parse_json(json)?;
        let miss = value
            .get("persona_miss")
            .ok_or_else(|| "fixture is missing persona_miss".to_string())?;
        let miss_id = miss
            .get("id")
            .and_then(Json::as_str)
            .ok_or_else(|| "persona_miss.id is missing".to_string())?
            .to_string();
        let miss_text = miss
            .get("text")
            .and_then(Json::as_str)
            .ok_or_else(|| "persona_miss.text is missing".to_string())?
            .to_string();
        if miss_id.is_empty() || miss_text.trim().is_empty() {
            return Err("persona_miss id and text must be non-empty".into());
        }
        let rows = value
            .get("clauses")
            .and_then(Json::as_array)
            .ok_or_else(|| "fixture is missing clauses".to_string())?;
        let mut clauses = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row
                .get("id")
                .and_then(Json::as_str)
                .ok_or_else(|| "clause id is missing".to_string())?
                .to_string();
            let text = row
                .get("text")
                .and_then(Json::as_str)
                .ok_or_else(|| format!("clause {id} text is missing"))?
                .to_string();
            let atomic = row.get("atomic").and_then(Json::as_bool).unwrap_or(false);
            if id.is_empty() || text.trim().is_empty() {
                return Err("a clause id and text must be non-empty".into());
            }
            if id == miss_id {
                return Err("persona_miss is not a retrieved clause".into());
            }
            clauses.push(StoredClause { id, text, atomic });
        }
        if clauses.is_empty() {
            return Err("fixture has no clauses".into());
        }
        Ok(Self {
            miss_id,
            miss_text,
            clauses,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number,
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text.as_str()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items.as_slice()),
            _ => None,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

fn parse_json(input: &str) -> Result<Json, String> {
    let mut parser = Parser {
        s: input.as_bytes(),
        i: 0,
    };
    let value = parser.value()?;
    parser.skip();
    if parser.i != parser.s.len() {
        return Err("fixture JSON has trailing bytes".into());
    }
    Ok(value)
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, byte: u8) -> Result<(), String> {
        match self.peek() {
            Some(found) if found == byte => {
                self.i += 1;
                Ok(())
            }
            _ => Err(format!("expected {}", byte as char)),
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'n') => self.literal(b"null", Json::Null),
            Some(b't') => self.literal(b"true", Json::Bool(true)),
            Some(b'f') => self.literal(b"false", Json::Bool(false)),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err("expected a JSON value".into()),
        }
    }

    fn literal(&mut self, word: &[u8], value: Json) -> Result<Json, String> {
        self.skip();
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(value)
        } else {
            Err("bad JSON literal".into())
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        self.skip();
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        if self.s.get(self.i).is_none_or(|b| !b.is_ascii_digit()) {
            return Err("bad JSON number".into());
        }
        while self.s.get(self.i).is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
        }
        if self.s.get(self.i) == Some(&b'.') {
            self.i += 1;
            if self.s.get(self.i).is_none_or(|b| !b.is_ascii_digit()) {
                return Err("bad JSON number".into());
            }
            while self.s.get(self.i).is_some_and(|b| b.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if start == self.i {
            return Err("bad JSON number".into());
        }
        Ok(Json::Number)
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        while self.i < self.s.len() {
            let byte = self.s[self.i];
            self.i += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = self
                        .s
                        .get(self.i)
                        .copied()
                        .ok_or("truncated string escape")?;
                    self.i += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'u' => {
                            let hex = self.s.get(self.i..self.i + 4).ok_or("short \\u escape")?;
                            let text = std::str::from_utf8(hex).map_err(|_| "bad \\u escape")?;
                            let code =
                                u32::from_str_radix(text, 16).map_err(|_| "bad \\u escape")?;
                            out.push(char::from_u32(code).ok_or("bad \\u codepoint")?);
                            self.i += 4;
                        }
                        _ => return Err("unknown string escape".into()),
                    }
                }
                c if c < 0x20 => return Err("raw control byte in string".into()),
                c => {
                    // UTF-8 continuation is copied with the leading byte.
                    if c < 0x80 {
                        out.push(c as char);
                    } else {
                        let width = if c & 0xE0 == 0xC0 {
                            2
                        } else if c & 0xF0 == 0xE0 {
                            3
                        } else if c & 0xF8 == 0xF0 {
                            4
                        } else {
                            return Err("bad utf-8 in string".into());
                        };
                        let start = self.i - 1;
                        let end = start + width;
                        let bytes = self.s.get(start..end).ok_or("truncated utf-8")?;
                        let text = std::str::from_utf8(bytes).map_err(|_| "bad utf-8 in string")?;
                        out.push_str(text);
                        self.i = end;
                    }
                }
            }
        }
        Err("unterminated string".into())
    }

    fn array(&mut self) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err("expected ',' or ']'".into()),
            }
        }
        Ok(Json::Array(items))
    }

    fn object(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err("object key must be a string".into());
            }
            let key = self.string()?;
            self.eat(b':')?;
            let value = self.value()?;
            pairs.push((key, value));
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err("expected ',' or '}'".into()),
            }
        }
        Ok(Json::Object(pairs))
    }
}
