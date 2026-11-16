// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Streaming read of one loopback chat completion.
//!
//! `stream: true` asks for server-sent deltas. A gateway that ignores the flag
//! and returns one JSON body is still one delta, at the end. The key never
//! leaves the request header.

use super::{
    assistant_content, find_header_end, is_chunked, json_string, parse_json_string, status_of,
    HermesTarget,
};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

pub(super) fn post_chat_stream(
    target: &HermesTarget,
    model: &str,
    key: &str,
    system: &str,
    user: &str,
    timeout: Duration,
    on_delta: &mut dyn FnMut(&str),
) -> Result<String, ()> {
    let body = format!(
        "{{\"model\":{model},\"stream\":true,\"messages\":[{{\"role\":\"system\",\"content\":{system}}},{{\"role\":\"user\",\"content\":{user}}}]}}",
        model = json_string(model),
        system = json_string(system),
        user = json_string(user),
    );
    let mut sock = TcpStream::connect_timeout(&target.addr, timeout).map_err(|_| ())?;
    let deadline = Instant::now() + timeout;
    let slice = timeout.max(Duration::from_millis(50));
    sock.set_read_timeout(Some(slice)).map_err(|_| ())?;
    sock.set_write_timeout(Some(timeout)).map_err(|_| ())?;
    let auth = if key.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {key}\r\n")
    };
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n{auth}\r\n{body}",
        path = target.path,
        addr = target.addr,
        len = body.len(),
        auth = auth,
        body = body,
    );
    sock.write_all(req.as_bytes()).map_err(|_| ())?;
    let mut parser = StreamParser::new();
    let mut tmp = [0u8; 4096];
    loop {
        if Instant::now() >= deadline {
            break;
        }
        match sock.read(&mut tmp) {
            Ok(0) => {
                parser.finish();
                break;
            }
            Ok(n) => {
                for piece in parser.push(&tmp[..n]) {
                    if !piece.is_empty() {
                        on_delta(&piece);
                    }
                }
                if parser.failed() {
                    return Err(());
                }
                if parser.done() {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }
    if parser.failed() || !parser.headers_ok() {
        return Err(());
    }
    if !parser.saw_sse() {
        let text = parser.json_content().ok_or(())?;
        if !text.is_empty() {
            on_delta(&text);
        }
        return Ok(text);
    }
    if parser.text().is_empty() {
        return Err(());
    }
    Ok(parser.text())
}

struct StreamParser {
    raw: Vec<u8>,
    header_end: Option<usize>,
    status: u16,
    failed: bool,
    mode: BodyMode,
    decoded: Vec<u8>,
    decoded_fed: usize,
    sse: Vec<u8>,
    saw_sse: bool,
    text: String,
    done: bool,
}

#[derive(Clone, Copy)]
enum BodyMode {
    Unknown,
    UntilClose,
    Length(usize),
    Chunked,
}

impl StreamParser {
    fn new() -> Self {
        Self {
            raw: Vec::new(),
            header_end: None,
            status: 0,
            failed: false,
            mode: BodyMode::Unknown,
            decoded: Vec::new(),
            decoded_fed: 0,
            sse: Vec::new(),
            saw_sse: false,
            text: String::new(),
            done: false,
        }
    }

    fn failed(&self) -> bool {
        self.failed
    }

    fn headers_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn saw_sse(&self) -> bool {
        self.saw_sse
    }

    fn text(&self) -> String {
        self.text.clone()
    }

    fn done(&self) -> bool {
        self.done
    }

    fn json_content(&self) -> Option<String> {
        let body = std::str::from_utf8(&self.decoded).ok()?;
        assistant_content(body)
    }

    fn finish(&mut self) {
        if self.header_end.is_none() {
            self.failed = true;
            return;
        }
        self.pull_body(true);
        if !self.saw_sse {
            self.done = true;
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        if self.raw.len() + chunk.len() > 256 * 1024 {
            self.failed = true;
            return Vec::new();
        }
        self.raw.extend_from_slice(chunk);
        if self.header_end.is_none() {
            let Some(end) = find_header_end(&self.raw) else {
                return Vec::new();
            };
            let head = &self.raw[..end];
            self.status = std::str::from_utf8(head).map(status_of).unwrap_or(0);
            if !(200..300).contains(&self.status) {
                self.failed = true;
                return Vec::new();
            }
            self.mode = if is_chunked(head) {
                BodyMode::Chunked
            } else if let Some(n) = super::content_length(head) {
                BodyMode::Length(n)
            } else {
                BodyMode::UntilClose
            };
            self.header_end = Some(end + 4);
        }
        self.pull_body(false)
    }

    fn pull_body(&mut self, eof: bool) -> Vec<String> {
        let start = self.header_end.unwrap_or(0);
        if start > self.raw.len() {
            return Vec::new();
        }
        let body = &self.raw[start..];
        match self.mode {
            BodyMode::Unknown => Vec::new(),
            BodyMode::UntilClose => {
                self.decoded = body.to_vec();
                if eof {
                    self.done = self.saw_sse;
                }
                self.feed_new()
            }
            BodyMode::Length(n) => {
                let take = n.min(body.len());
                self.decoded = body[..take].to_vec();
                if take >= n {
                    self.done = true;
                }
                self.feed_new()
            }
            BodyMode::Chunked => {
                let (all, finished) = decode_chunked_prefix(body);
                if all.len() < self.decoded_fed {
                    return Vec::new();
                }
                self.decoded = all;
                if finished {
                    self.done = true;
                }
                self.feed_new()
            }
        }
    }

    fn feed_new(&mut self) -> Vec<String> {
        if self.decoded.len() < self.decoded_fed {
            return Vec::new();
        }
        let new = self.decoded[self.decoded_fed..].to_vec();
        self.decoded_fed = self.decoded.len();
        if new.is_empty() {
            return Vec::new();
        }
        self.sse.extend_from_slice(&new);
        self.drain_sse()
    }

    fn drain_sse(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            let Some(nl) = self.sse.iter().position(|&b| b == b'\n') else {
                break;
            };
            let mut line: Vec<u8> = self.sse.drain(..=nl).collect();
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(rest) = line.strip_prefix("data:") else {
                continue;
            };
            self.saw_sse = true;
            let rest = rest.trim();
            if rest == "[DONE]" {
                self.done = true;
                break;
            }
            if let Some(piece) = delta_content(rest) {
                if !piece.is_empty() {
                    self.text.push_str(&piece);
                    out.push(piece);
                }
            }
        }
        out
    }
}

fn delta_content(json: &str) -> Option<String> {
    let idx = json.find("\"content\"")?;
    let after = json[idx + "\"content\"".len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if after.starts_with("null") {
        return None;
    }
    parse_json_string(after)
}

/// Decode complete chunks from `raw`. The bool is true once the zero chunk arrives.
fn decode_chunked_prefix(raw: &[u8]) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let Some(rel) = raw[i..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let hex = std::str::from_utf8(&raw[i..i + rel]).unwrap_or("").trim();
        let hex = hex.split(';').next().unwrap_or("").trim();
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            break;
        }
        let Ok(n) = usize::from_str_radix(hex, 16) else {
            break;
        };
        let data_at = i + rel + 2;
        if n == 0 {
            return (out, true);
        }
        if data_at + n + 2 > raw.len() {
            break;
        }
        out.extend_from_slice(&raw[data_at..data_at + n]);
        i = data_at + n + 2;
        if out.len() > 64 * 1024 {
            break;
        }
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_deltas_emit_as_the_bytes_arrive() {
        let mut p = StreamParser::new();
        let head =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        assert!(p.push(head).is_empty());
        let one = b"data: {\"choices\":[{\"delta\":{\"content\":\"She died this morning.\"}}]}\n\n";
        assert_eq!(p.push(one), vec!["She died this morning.".to_string()]);
        let two = b"data: {\"choices\":[{\"delta\":{\"content\":\" I will stay.\"}}]}\n\ndata: [DONE]\n\n";
        assert_eq!(p.push(two), vec![" I will stay.".to_string()]);
        assert!(p.done());
        assert_eq!(p.text(), "She died this morning. I will stay.");
    }

    #[test]
    fn chunked_sse_decodes_before_the_zero_chunk() {
        let mut p = StreamParser::new();
        let head = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(p.push(head).is_empty());
        let payload = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi.\"}}]}\n\n";
        let chunk = format!(
            "{:x}\r\n{}\r\n",
            payload.len(),
            std::str::from_utf8(payload).unwrap()
        );
        assert_eq!(p.push(chunk.as_bytes()), vec!["Hi.".to_string()]);
        assert!(!p.done());
        assert_eq!(p.push(b"0\r\n\r\n"), Vec::<String>::new());
        assert!(p.done());
    }

    #[test]
    fn json_body_is_one_delta_at_the_end() {
        let payload = "{\"choices\":[{\"message\":{\"content\":\"She died this morning.\"}}]}";
        let raw = format!(
            "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let mut p = StreamParser::new();
        assert!(p.push(raw.as_bytes()).is_empty());
        assert!(!p.saw_sse());
        assert_eq!(p.json_content().as_deref(), Some("She died this morning."));
    }
}
