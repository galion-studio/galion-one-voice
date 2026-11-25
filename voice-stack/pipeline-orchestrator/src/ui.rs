// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback one-dot page. It subscribes by polling [`crate::Subscribe`].
//!
//! Bind policy matches `galion-render`: a non-loopback address is refused.
//! This process does not set a public-bind flag. Nothing here is installed
//! into the systemd stage unit.

use crate::pipeline::{MoodSnap, Phase, Subscribe};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../ui/index.html");

pub struct DotServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DotServer {
    /// `127.0.0.1` or `::1` only. Port `0` picks a free port.
    ///
    /// The page name is the configured persona id: `GALION_HORBM_DEV_PERSONA`
    /// when that value is a non-empty id, otherwise `development_persona` in
    /// the boot file. Nothing here names a persona.
    pub fn bind(addr: SocketAddr, sub: Subscribe) -> Result<Self, String> {
        if !addr.ip().is_loopback() {
            return Err(LOOPBACK.into());
        }
        Self::bind_named(addr, sub, configured_display_name())
    }

    /// [`Self::bind`] with the display name the caller already resolved.
    pub fn bind_named(
        addr: SocketAddr,
        sub: Subscribe,
        name: impl AsRef<str>,
    ) -> Result<Self, String> {
        if !addr.ip().is_loopback() {
            return Err(LOOPBACK.into());
        }
        let page = page_html(name.as_ref());
        let listener = TcpListener::bind(addr).map_err(|err| err.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let addr = listener.local_addr().map_err(|err| err.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::spawn(move || serve(listener, sub, flag, page));
        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for DotServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const LOOPBACK: &str = "one-dot UI is loopback only; refusing non-loopback bind";
const NAME_MARK: &str = "GALION_DOT_NAME";
/// Same env var as `horbm_persona::BootConfig::ENV`.
const DEV_PERSONA_ENV: &str = "GALION_HORBM_DEV_PERSONA";
/// Relative boot files. Same search as `BootConfig`, and none is absolute.
const BOOT_REL: &[&str] = &[
    "development.toml",
    "horbm-persona/development.toml",
    "crates/horbm-persona/development.toml",
    "horbm/crates/horbm-persona/development.toml",
];

fn serve(listener: TcpListener, sub: Subscribe, stop: Arc<AtomicBool>, page: String) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_client(stream, &sub, &page);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_client(mut stream: TcpStream, sub: &Subscribe, page: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    let path = request_path(&head);
    let (status, content_type, body) = match path {
        Some("/" | "/index.html") => ("200 OK", "text/html; charset=utf-8", page.to_string()),
        // `phase()` is `watch::Receiver::borrow()`: the value stored in the
        // channel at this read, not a copy taken when the server subscribed.
        Some("/state" | "/dot") => (
            "200 OK",
            "application/json",
            state_json(sub.phase(), sub.mood()),
        ),
        Some(_) => ("404 Not Found", "text/plain", "not found".into()),
        None => ("400 Bad Request", "text/plain", "bad request".into()),
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    Ok(())
}

fn request_path(head: &str) -> Option<&str> {
    let line = head.lines().next()?.trim();
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    let raw = parts.next()?;
    Some(raw.split('?').next().unwrap_or(raw))
}

pub fn state_json(phase: Phase, snap: MoodSnap) -> String {
    format!(
        "{{\"phase\":\"{}\",\"mood\":{{\"arousal\":{:.4},\"valence\":{:.4},\"dominance\":{:.4}}},\"frame\":{},\"advance_us\":{}}}",
        phase.as_str(),
        snap.pad.arousal,
        snap.pad.valence,
        snap.pad.dominance,
        snap.frame,
        snap.advance_us
    )
}

/// Configured persona id. The env var wins when it is a non-empty id.
/// Otherwise the boot file's `development_persona` key does. A missing
/// source is an empty name, not a built-in persona.
fn configured_display_name() -> String {
    if let Some(id) = env_persona_id() {
        return id;
    }
    boot_persona_id().unwrap_or_default()
}

fn env_persona_id() -> Option<String> {
    let raw = std::env::var(DEV_PERSONA_ENV).ok()?;
    let id = raw.trim();
    if id.is_empty() {
        None
    } else {
        token_id(id)
    }
}

fn boot_persona_id() -> Option<String> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..32 {
        for rel in BOOT_REL {
            let path = dir.join(rel);
            if !path.is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(id) = persona_id_in_boot(&text) {
                return Some(id);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn persona_id_in_boot(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = strip_hash_comment(line).trim();
        let Some(rest) = line.strip_prefix("development_persona") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        if let Some(id) = token_id(unquote(rest.trim())) {
            return Some(id);
        }
    }
    None
}

fn strip_hash_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b'#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}

/// Id token. Same shape `horbm-persona` accepts: 1 to 16 bytes, a leading
/// lowercase letter, then lowercase letters, digits, or `-`.
fn token_id(id: &str) -> Option<String> {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > 16 || !bytes[0].is_ascii_lowercase() {
        return None;
    }
    if bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        Some(id.to_string())
    } else {
        None
    }
}

fn page_html(name: &str) -> String {
    INDEX_HTML.replace(NAME_MARK, &html_escape(name))
}

fn html_escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn refuses_a_public_bind() {
        let (phase_tx, phase_rx) = tokio::sync::watch::channel(Phase::Idle);
        let (mood_tx, mood_rx) = tokio::sync::watch::channel(MoodSnap {
            pad: crate::Pad::RESTING,
            frame: 0,
            advance_us: 0,
            at_ms: 0,
        });
        let sub = Subscribe::from_receivers(phase_rx, mood_rx);
        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9));
        let err = match DotServer::bind(addr, sub) {
            Err(err) => err,
            Ok(server) => {
                server.shutdown();
                panic!("public bind was accepted");
            }
        };
        assert!(err.contains("loopback"), "{err}");
        drop(phase_tx);
        drop(mood_tx);
    }

    #[test]
    fn page_inserts_the_given_name() {
        let page = page_html("northwind");
        assert!(page.contains("<title>northwind</title>"));
        assert!(page.contains("<div class=\"name\">northwind</div>"));
        assert!(!page.contains(NAME_MARK));
        assert_eq!(page, INDEX_HTML.replace(NAME_MARK, "northwind"));
    }

    #[test]
    fn page_escapes_a_name() {
        let page = page_html("<nw>");
        assert!(page.contains("&lt;nw&gt;"));
        assert!(!page.contains("<nw>"));
    }

    #[test]
    fn boot_text_yields_the_development_persona_id() {
        let text = "# note\ndevelopment_persona = \"northwind\"\npersona_dir = \"personas\"\n";
        assert_eq!(persona_id_in_boot(text).as_deref(), Some("northwind"));
        assert!(persona_id_in_boot("persona_dir = \"personas\"\n").is_none());
        assert!(persona_id_in_boot("development_persona = \"\"\n").is_none());
        assert!(persona_id_in_boot("# development_persona = \"northwind\"\n").is_none());
    }
}
