// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback one-dot page. The dot is `#F5F5F7` scaled by brightness. The wash
//! is background only, the same split as `galion-render` (`palette::DOT` vs
//! the obsidian / amber / violet ground). Nothing here binds a public address.

use crate::error::{EmotionError, EmotionErrorKind};
use crate::report::{self, Report};
use crate::stream::Reading;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

const INDEX: &str = include_str!("../ui/index.html");
const DOT_JS: &str = include_str!("../ui/dot.js");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Publish {
    pub reading: Reading,
    pub push_median_us: u32,
    pub push_p95_us: u32,
    pub dropped_tail_samples: u32,
    pub frames: u32,
}

impl Publish {
    pub const EMPTY: Publish = Publish {
        reading: Reading::EMPTY,
        push_median_us: 0,
        push_p95_us: 0,
        dropped_tail_samples: 0,
        frames: 0,
    };

    fn as_report(self) -> Report {
        Report {
            reading: self.reading,
            frames: self.frames,
            dropped_tail_samples: self.dropped_tail_samples,
            audio_retained_samples: 0,
            push_median_us: self.push_median_us,
            push_p95_us: self.push_p95_us,
        }
    }
}

/// Refuse anything that is not a loopback address. Does not bind those.
pub fn bind_loopback(addr: &str) -> Result<TcpListener, EmotionError> {
    let sock: SocketAddr = addr.parse().map_err(|_| {
        EmotionError::new(
            EmotionErrorKind::Bind,
            "address must look like 127.0.0.1:8768",
        )
    })?;
    if !sock.ip().is_loopback() {
        return Err(EmotionError::new(EmotionErrorKind::Bind, "loopback only"));
    }
    TcpListener::bind(sock).map_err(|_| EmotionError::new(EmotionErrorKind::Bind, "bind failed"))
}

pub fn handle_client(stream: &mut TcpStream, publish: &Publish) -> std::io::Result<()> {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf)?;
    let text = String::from_utf8_lossy(&buf[..n]);
    let path = request_path(&text);
    let report = publish.as_report();
    match path {
        "/" | "/index.html" => respond(stream, "200 OK", "text/html; charset=utf-8", INDEX),
        "/dot.js" => respond(stream, "200 OK", "text/javascript; charset=utf-8", DOT_JS),
        "/reading" => respond(
            stream,
            "200 OK",
            "application/json",
            &report::report_json(&report),
        ),
        "/dot" => respond(
            stream,
            "200 OK",
            "application/json",
            &report::dot_json(&publish.reading, Some(&report)),
        ),
        _ => respond(
            stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found\n",
        ),
    }
}

fn request_path(req: &str) -> &str {
    let mut parts = req.split_whitespace();
    let _method = parts.next();
    let raw = parts.next().unwrap_or("/");
    raw.split('?').next().unwrap_or("/")
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn public_addresses_are_refused_before_bind() {
        let err = bind_loopback("0.0.0.0:9").unwrap_err();
        assert_eq!(err.kind, EmotionErrorKind::Bind);
        assert!(err.detail.contains("loopback"));
        let err = bind_loopback("8.8.8.8:80").unwrap_err();
        assert_eq!(err.kind, EmotionErrorKind::Bind);
    }

    #[test]
    fn dot_route_is_one_json_object() {
        let listener = bind_loopback("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let publish = Publish::EMPTY;
        let server = thread::spawn(move || {
            let (mut client, _) = listener.accept().unwrap();
            client.set_read_timeout(Some(Duration::from_secs(2))).ok();
            handle_client(&mut client, &publish).unwrap();
        });
        let mut sock = TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).ok();
        sock.write_all(b"GET /dot HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        sock.read_to_string(&mut body).unwrap();
        server.join().unwrap();
        assert!(body.contains("200 OK"), "{body}");
        assert!(body.contains("\"state\":\"idle\""), "{body}");
        assert!(body.contains("\"audio_retained_samples\":0"), "{body}");
        assert!(body.contains("#F5F5F7") || body.contains("\"brightness\""));
        let json = body.split("\r\n\r\n").nth(1).unwrap_or("");
        assert!(json.starts_with('{') && json.ends_with('}'), "{json}");
    }
}
