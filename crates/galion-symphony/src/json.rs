// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! A small JSON reader for safetensors headers. Objects keep source order.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    /// Digits as written, so integer shapes stay exact.
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn as_obj(&self) -> Result<&[(String, J)]> {
        match self {
            J::Obj(o) => Ok(o),
            _ => Err(Error::Bad("json: expected object".into())),
        }
    }

    pub fn get<'a>(&'a self, key: &str) -> Option<&'a J> {
        let J::Obj(o) = self else { return None };
        o.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn as_str(&self) -> Result<&str> {
        match self {
            J::Str(s) => Ok(s),
            _ => Err(Error::Bad("json: expected string".into())),
        }
    }

    pub fn as_u64(&self) -> Result<u64> {
        match self {
            J::Num(s) if !s.contains(['.', 'e', 'E', '+']) => s
                .parse()
                .map_err(|_| Error::Bad(format!("json: bad integer {s}"))),
            _ => Err(Error::Bad("json: expected unsigned integer".into())),
        }
    }

    pub fn as_arr(&self) -> Result<&[J]> {
        match self {
            J::Arr(a) => Ok(a),
            _ => Err(Error::Bad("json: expected array".into())),
        }
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

pub fn parse(text: &str) -> Result<J> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
    };
    p.skip();
    let v = p.value()?;
    p.skip();
    if p.i != p.s.len() {
        return Err(Error::Bad("json: trailing bytes".into()));
    }
    Ok(v)
}

impl<'a> P<'a> {
    fn skip(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Result<u8> {
        self.skip();
        self.s
            .get(self.i)
            .copied()
            .ok_or_else(|| Error::Bad("json: truncated".into()))
    }

    fn bump(&mut self) -> Result<u8> {
        let c = self.peek()?;
        self.i += 1;
        Ok(c)
    }

    fn value(&mut self) -> Result<J> {
        match self.peek()? {
            b'n' => self.lit(b"null", J::Null),
            b't' => self.lit(b"true", J::Bool(true)),
            b'f' => self.lit(b"false", J::Bool(false)),
            b'"' => Ok(J::Str(self.string()?)),
            b'[' => self.array(),
            b'{' => self.object(),
            b'-' | b'0'..=b'9' => self.number(),
            c => Err(Error::Bad(format!("json: unexpected byte {c}"))),
        }
    }

    fn lit(&mut self, word: &[u8], v: J) -> Result<J> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(Error::Bad("json: bad literal".into()))
        }
    }

    fn number(&mut self) -> Result<J> {
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        if self.i >= self.s.len() || !self.s[self.i].is_ascii_digit() {
            return Err(Error::Bad("json: bad number".into()));
        }
        while self.i < self.s.len()
            && matches!(
                self.s[self.i],
                b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
            )
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.s[start..self.i])
            .map_err(|_| Error::Bad("json: number utf8".into()))?;
        Ok(J::Num(s.to_owned()))
    }

    fn string(&mut self) -> Result<String> {
        if self.bump()? != b'"' {
            return Err(Error::Bad("json: string".into()));
        }
        let mut out = Vec::new();
        loop {
            if self.i >= self.s.len() {
                return Err(Error::Bad("json: truncated string".into()));
            }
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self
                        .s
                        .get(self.i)
                        .copied()
                        .ok_or_else(|| Error::Bad("json: bad escape".into()))?;
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return Err(Error::Bad("json: short unicode escape".into()));
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4])
                                .map_err(|_| Error::Bad("json: unicode escape".into()))?;
                            self.i += 4;
                            let u = u32::from_str_radix(hex, 16)
                                .map_err(|_| Error::Bad("json: unicode escape".into()))?;
                            let ch = char::from_u32(u)
                                .ok_or_else(|| Error::Bad("json: unicode escape".into()))?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(Error::Bad("json: bad escape".into())),
                    }
                }
                0x00..=0x1f => return Err(Error::Bad("json: raw control in string".into())),
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| Error::Bad("json: string utf8".into()))
    }

    fn array(&mut self) -> Result<J> {
        self.bump()?;
        let mut items = Vec::new();
        self.skip();
        if self.peek()? == b']' {
            self.i += 1;
            return Ok(J::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip();
            match self.bump()? {
                b']' => break,
                b',' => continue,
                _ => return Err(Error::Bad("json: array".into())),
            }
        }
        Ok(J::Arr(items))
    }

    fn object(&mut self) -> Result<J> {
        self.bump()?;
        let mut items = Vec::new();
        self.skip();
        if self.peek()? == b'}' {
            self.i += 1;
            return Ok(J::Obj(items));
        }
        loop {
            self.skip();
            if self.peek()? != b'"' {
                return Err(Error::Bad("json: object key".into()));
            }
            let k = self.string()?;
            self.skip();
            if self.bump()? != b':' {
                return Err(Error::Bad("json: object colon".into()));
            }
            let v = self.value()?;
            items.push((k, v));
            self.skip();
            match self.bump()? {
                b'}' => break,
                b',' => continue,
                _ => return Err(Error::Bad("json: object".into())),
            }
        }
        Ok(J::Obj(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_header_shape() {
        let v = parse(r#"{"__metadata__":{"format":"pt"},"a.b":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]}}"#).unwrap();
        assert_eq!(
            v.get("a.b")
                .unwrap()
                .get("dtype")
                .unwrap()
                .as_str()
                .unwrap(),
            "F32"
        );
        let shape = v
            .get("a.b")
            .unwrap()
            .get("shape")
            .unwrap()
            .as_arr()
            .unwrap();
        assert_eq!(shape[0].as_u64().unwrap(), 2);
        assert_eq!(
            v.get("__metadata__")
                .unwrap()
                .get("format")
                .unwrap()
                .as_str()
                .unwrap(),
            "pt"
        );
    }
}
