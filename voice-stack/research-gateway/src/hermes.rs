// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Hermes on this machine, and nowhere else.
//!
//! Galion Research today is the Node desk on `127.0.0.1:8787`. That port
//! clashes with the HORBM preview, so this crate does not bind it. The desk's
//! model client (`hermes-research` `src/hermes.js` at `13ff2938`) dials
//! `http://127.0.0.1:8642/v1/chat/completions` and refuses every other host.
//! This module is that same dial, with the voice cap of 3 seconds.
//!
//! No key is read and no `Authorization` header is sent. A refused or late
//! Hermes is no answer. The router then says "I don't know."

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::{read_http, LocalAnswer, Lookup};

/// The only host this process dials.
pub const HERMES_ADDR: &str = "127.0.0.1:8642";

pub const HERMES_PORT: u16 = 8642;

pub const HERMES_PATH: &str = "/v1/chat/completions";

/// Same model name as `hermes.js` `LLM_NAME`.
pub const HERMES_MODEL: &str = "hermes-agent";

const HERMES_SYSTEM: &str = "\
You are Galion Research on this machine. Answer in one or two English sentences. \
If you do not know, say I don't know. Do not invent a paper, a URL, a date, or a measurement. \
Do not call a remote host or a paid API.";

const MAX_QUESTION_CHARS: usize = 2_000;
const MAX_REPLY_BYTES: usize = 64 * 1024;

/// Dial [`HERMES_ADDR`] only. `cap` is the socket deadline for this ask.
#[derive(Clone, Copy, Debug)]
pub struct HermesLookup {
    cap: Duration,
}

impl HermesLookup {
    pub fn new(cap: Duration) -> Self {
        Self { cap }
    }

    pub fn cap(&self) -> Duration {
        self.cap
    }

    /// The socket this lookup opens. It is not a parameter.
    pub fn socket() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), HERMES_PORT)
    }
}

impl Lookup for HermesLookup {
    fn lookup(&self, question: &str) -> Option<LocalAnswer> {
        let text = ask(question, self.cap)?;
        Some(LocalAnswer {
            text,
            source: "hermes".to_string(),
        })
    }
}

/// True only for `127.0.0.1:8642`. `8787`, `::1`, and every other port are false.
pub fn is_hermes_socket(addr: SocketAddr) -> bool {
    addr.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST) && addr.port() == HERMES_PORT
}

fn ask(question: &str, cap: Duration) -> Option<String> {
    let question = clip_chars(question.trim(), MAX_QUESTION_CHARS);
    if question.is_empty() || cap.is_zero() {
        return None;
    }
    let addr = HermesLookup::socket();
    if !is_hermes_socket(addr) {
        return None;
    }
    let mut stream = TcpStream::connect_timeout(&addr, cap).ok()?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(cap));
    let _ = stream.set_write_timeout(Some(cap));
    let payload = serde_json::json!({
        "model": HERMES_MODEL,
        "messages": [
            {"role": "system", "content": HERMES_SYSTEM},
            {"role": "user", "content": question},
        ]
    })
    .to_string();
    let req = format!(
        "POST {HERMES_PATH} HTTP/1.1\r\nHost: {HERMES_ADDR}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).ok()?;
    let _ = stream.flush();
    let (header, body) = read_http(&mut stream, MAX_REPLY_BYTES).ok()?;
    let status = header.lines().next().unwrap_or("");
    if !status.contains(" 200 ") {
        return None;
    }
    let content = assistant_content(&body)?;
    hermes_spoken(&content)
}

/// Text we will hand the router. A URL, an empty line, or "I don't know" is dropped.
pub fn hermes_spoken(content: &str) -> Option<String> {
    let text = content.trim();
    if text.is_empty() || text.contains("://") || text.contains('@') {
        return None;
    }
    let folded = text
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' => '\'',
            other => other,
        })
        .collect::<String>()
        .to_lowercase();
    if folded == "i don't know" || folded == "i don't know." {
        return None;
    }
    Some(text.to_string())
}

fn assistant_content(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    match value.pointer("/choices/0/message/content")? {
        serde_json::Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn clip_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
