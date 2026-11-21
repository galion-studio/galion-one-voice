// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 11, the Galion Research gateway.
//!
//! Titan is gone. This crate is the local Rust replacement. It binds
//! `127.0.0.1` and nothing else. The Node desk (`hermes-research`) already
//! listens on `127.0.0.1:8787`, and that port clashes with the HORBM preview,
//! so this process does not use 8787.
//!
//! The only outbound call is Hermes at `127.0.0.1:8642`
//! (`/v1/chat/completions`). There is no other host, no paid API, and no key
//! in this process. [`NoRemoteLookup`] is the test double that does not open
//! a socket. The binary uses [`HermesLookup`].
//!
//! The Samantha turn calls this only after HORBM cards miss, and only for a
//! deep question. The cap is [`RESEARCH_CAP`] (3 seconds). A late body is
//! dropped. A Hermes miss, a timeout, or an unreachable port becomes
//! "I don't know." in the router.
//!
//! Port [`DEFAULT_PORT`] (7911) sits in 7900–7999. It is not 7881, 7878,
//! 8787, or 11434, and it is not 8642.

#![forbid(unsafe_code)]

mod hermes;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Hard cap on one research lookup. Same 3 seconds as `galion_sideband::RETRIEVAL_CAP`.
pub const RESEARCH_CAP: Duration = Duration::from_secs(3);

pub use hermes::{
    hermes_spoken, is_hermes_socket, HermesLookup, HERMES_ADDR, HERMES_MODEL, HERMES_PATH,
    HERMES_PORT,
};

/// Documented listen port. `127.0.0.1` only.
pub const DEFAULT_PORT: u16 = 7911;

/// Documented bind string. Hostname `localhost` is rejected so nothing resolves.
pub const DEFAULT_BIND: &str = "127.0.0.1:7911";

const MAX_HTTP_BYTES: usize = 8 * 1024;

/// Why a bind or a client address was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayError {
    /// The address was not `127.0.0.1`. `0.0.0.0`, `::1`, and other hosts are refused.
    NotLoopback,
    /// Not an `ip:port` literal.
    BadAddr,
    Io(String),
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatewayError::NotLoopback => {
                write!(f, "research gateway binds 127.0.0.1 only")
            }
            GatewayError::BadAddr => {
                write!(f, "research gateway address must be 127.0.0.1 and a port")
            }
            GatewayError::Io(msg) => write!(f, "research gateway: {msg}"),
        }
    }
}

impl std::error::Error for GatewayError {}

/// One local answer. `source` is a short label such as `local`, not a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAnswer {
    pub text: String,
    pub source: String,
}

/// What the process may consult. Implementations must not open a remote socket.
pub trait Lookup: Send + Sync {
    fn lookup(&self, question: &str) -> Option<LocalAnswer>;
}

/// The landed default. Titan is gone, so there is no remote corpus to ask.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRemoteLookup;

impl Lookup for NoRemoteLookup {
    fn lookup(&self, _question: &str) -> Option<LocalAnswer> {
        None
    }
}

/// Accept `127.0.0.1:<port>` only. Port `0` is the ephemeral port tests use.
pub fn parse_loopback(addr: &str) -> Result<SocketAddr, GatewayError> {
    let parsed: SocketAddr = addr.parse().map_err(|_| GatewayError::BadAddr)?;
    if !is_localhost_v4(parsed.ip()) {
        return Err(GatewayError::NotLoopback);
    }
    Ok(parsed)
}

fn is_localhost_v4(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4 == Ipv4Addr::LOCALHOST,
        IpAddr::V6(_) => false,
    }
}

/// Running gateway. Drop stops the accept loop.
pub struct Server {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Server {
    /// Bind and serve. `cap` is the longest a lookup may run before the
    /// response is `timeout`. The binary passes [`RESEARCH_CAP`].
    pub fn spawn(bind: &str, lookup: Arc<dyn Lookup>, cap: Duration) -> Result<Self, GatewayError> {
        let addr = parse_loopback(bind)?;
        let listener = TcpListener::bind(addr).map_err(|err| GatewayError::Io(err.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| GatewayError::Io(err.to_string()))?;
        let local = listener
            .local_addr()
            .map_err(|err| GatewayError::Io(err.to_string()))?;
        if !is_localhost_v4(local.ip()) {
            return Err(GatewayError::NotLoopback);
        }
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let join = thread::Builder::new()
            .name("galion-research".into())
            .spawn(move || accept_loop(listener, lookup, cap, flag))
            .map_err(|err| GatewayError::Io(err.to_string()))?;
        Ok(Self {
            addr: local,
            shutdown,
            join: Some(join),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    lookup: Arc<dyn Lookup>,
    cap: Duration,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let lookup = Arc::clone(&lookup);
                let _ = thread::Builder::new()
                    .name("galion-research-req".into())
                    .spawn(move || {
                        let _ = handle_conn(stream, lookup, cap);
                    });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

fn handle_conn(
    mut stream: TcpStream,
    lookup: Arc<dyn Lookup>,
    cap: Duration,
) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(cap));
    let _ = stream.set_write_timeout(Some(cap));
    let _ = stream.set_nodelay(true);
    let (header, body) = match read_http(&mut stream, MAX_HTTP_BYTES) {
        Ok(parts) => parts,
        Err(_) => return Ok(()),
    };
    let line = header.lines().next().unwrap_or("");
    if !line.starts_with("POST /v1/lookup HTTP/") {
        return write_json(&mut stream, 404, "Not Found", r#"{"status":"not_found"}"#);
    }
    let question = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value
            .get("question")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        Err(_) => String::new(),
    };
    if question.is_empty() {
        return write_json(&mut stream, 200, "OK", r#"{"status":"not_found"}"#);
    }
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(lookup.lookup(&question));
    });
    let reply = match rx.recv_timeout(cap) {
        Ok(Some(answer)) => match accept_answer(&answer) {
            Some(clean) => serde_json::json!({
                "status": "answer",
                "text": clean.text,
                "source": clean.source,
            })
            .to_string(),
            None => r#"{"status":"not_found"}"#.to_string(),
        },
        Ok(None) => r#"{"status":"not_found"}"#.to_string(),
        Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
            r#"{"status":"timeout"}"#.to_string()
        }
    };
    write_json(&mut stream, 200, "OK", &reply)
}

fn accept_answer(answer: &LocalAnswer) -> Option<LocalAnswer> {
    let source = clean_source(&answer.source)?;
    let text = answer.text.trim();
    if text.is_empty() {
        return None;
    }
    Some(LocalAnswer {
        text: text.to_string(),
        source,
    })
}

/// A source label. A URL or a host is refused so a lookup cannot name a remote.
pub fn clean_source(source: &str) -> Option<String> {
    if source.is_empty() || source.len() > 64 {
        return None;
    }
    if source.contains("://") || source.contains('/') || source.contains('@') {
        return None;
    }
    if source
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
    {
        Some(source.to_string())
    } else {
        None
    }
}

fn write_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> std::io::Result<()> {
    let msg = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(msg.as_bytes())?;
    let _ = stream.flush();
    Ok(())
}

/// What one client call returned. A timeout drops any late text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientReply {
    Answer { text: String, source: String },
    NotFound,
    Timeout,
    Unreachable,
}

/// Client of the localhost gateway. The socket opens on [`Self::ask_within`].
#[derive(Debug, Clone, Copy)]
pub struct ResearchClient {
    addr: SocketAddr,
    cap: Duration,
}

impl ResearchClient {
    /// Check the address. Does not connect yet, and refuses anything but `127.0.0.1`.
    pub fn connect(addr: &str) -> Result<Self, GatewayError> {
        let addr = parse_loopback(addr)?;
        Ok(Self {
            addr,
            cap: RESEARCH_CAP,
        })
    }

    pub fn with_cap(mut self, cap: Duration) -> Self {
        self.cap = cap;
        self
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn cap(&self) -> Duration {
        self.cap
    }

    pub fn ask(&self, question: &str) -> ClientReply {
        self.ask_within(question, self.cap)
    }

    /// `cap` is the hard deadline for this call, including the read.
    pub fn ask_within(&self, question: &str, cap: Duration) -> ClientReply {
        let started = Instant::now();
        let mut stream = match TcpStream::connect(self.addr) {
            Ok(stream) => stream,
            Err(_) => return ClientReply::Unreachable,
        };
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(cap));
        let _ = stream.set_write_timeout(Some(cap));
        let body = serde_json::json!({ "question": question }).to_string();
        let req = format!(
            "POST /v1/lookup HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(req.as_bytes()).is_err() {
            return if started.elapsed() > cap {
                ClientReply::Timeout
            } else {
                ClientReply::Unreachable
            };
        }
        let response = match read_http(&mut stream, MAX_HTTP_BYTES) {
            Ok((_, bytes)) => bytes,
            Err(err)
                if err.kind() == std::io::ErrorKind::TimedOut
                    || err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                return ClientReply::Timeout;
            }
            Err(_) if started.elapsed() >= cap => return ClientReply::Timeout,
            Err(_) => return ClientReply::Unreachable,
        };
        if started.elapsed() > cap {
            return ClientReply::Timeout;
        }
        parse_reply(&response)
    }
}

fn parse_reply(body: &[u8]) -> ClientReply {
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return ClientReply::Unreachable,
    };
    match value.get("status").and_then(|s| s.as_str()) {
        Some("not_found") => ClientReply::NotFound,
        Some("timeout") => ClientReply::Timeout,
        Some("answer") => {
            let text = value
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim();
            let source = value.get("source").and_then(|s| s.as_str()).unwrap_or("");
            match (text.is_empty(), clean_source(source)) {
                (false, Some(source)) => ClientReply::Answer {
                    text: text.to_string(),
                    source,
                },
                _ => ClientReply::NotFound,
            }
        }
        _ => ClientReply::Unreachable,
    }
}

pub(crate) fn read_http(
    stream: &mut TcpStream,
    cap_bytes: usize,
) -> std::io::Result<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let header_end = loop {
        if buf.len() > cap_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "research http too large",
            ));
        }
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "research http closed",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };
    let header = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut body = buf[header_end + 4..].to_vec();
    let need = content_length(&header).unwrap_or(0);
    if need > cap_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "research body too large",
        ));
    }
    while body.len() < need {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
        if body.len() > cap_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "research body too large",
            ));
        }
    }
    if body.len() > need {
        body.truncate(need);
    }
    Ok((header, body))
}

fn content_length(header: &str) -> Option<usize> {
    for line in header.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_port_is_localhost_in_the_free_band() {
        assert_eq!(DEFAULT_BIND, "127.0.0.1:7911");
        assert_eq!(DEFAULT_PORT, 7911);
        assert!((7900..=7999).contains(&DEFAULT_PORT));
        let taken = [7878_u16, 7879, 7880, 7881, 8642, 8766, 8787, 11434];
        assert!(!taken.contains(&DEFAULT_PORT));
        assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
        let addr = parse_loopback(DEFAULT_BIND).unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(addr.port(), 7911);
    }

    #[test]
    fn rejects_every_bind_that_is_not_127_0_0_1() {
        assert_eq!(
            parse_loopback("0.0.0.0:7911"),
            Err(GatewayError::NotLoopback)
        );
        assert_eq!(
            parse_loopback("127.0.0.2:7911"),
            Err(GatewayError::NotLoopback)
        );
        assert_eq!(parse_loopback("[::1]:7911"), Err(GatewayError::NotLoopback));
        assert_eq!(
            parse_loopback("192.0.2.1:7911"),
            Err(GatewayError::NotLoopback)
        );
        assert_eq!(parse_loopback("localhost:7911"), Err(GatewayError::BadAddr));
        assert!(matches!(
            Server::spawn("0.0.0.0:7911", Arc::new(NoRemoteLookup), RESEARCH_CAP),
            Err(GatewayError::NotLoopback)
        ));
        assert!(matches!(
            ResearchClient::connect("192.0.2.1:7911"),
            Err(GatewayError::NotLoopback)
        ));
    }

    #[test]
    fn source_label_cannot_be_a_url() {
        assert_eq!(clean_source("local").as_deref(), Some("local"));
        assert!(clean_source("https://example.test/a").is_none());
        assert!(clean_source("user@host").is_none());
    }

    #[test]
    fn spawn_binds_127_0_0_1_and_a_closed_port_is_unreachable() {
        let server = Server::spawn("127.0.0.1:0", Arc::new(NoRemoteLookup), RESEARCH_CAP).unwrap();
        let addr = server.addr();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(addr.port(), 0);
        let client = ResearchClient::connect(&addr.to_string()).unwrap();
        assert_eq!(client.ask("why is the tide late?"), ClientReply::NotFound);
        let closed = ResearchClient::connect("127.0.0.1:1").unwrap();
        assert_eq!(closed.ask("why?"), ClientReply::Unreachable);
    }
}
