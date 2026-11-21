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
    pub fn bind(addr: SocketAddr, sub: Subscribe) -> Result<Self, String> {
        if !addr.ip().is_loopback() {
            return Err("one-dot UI is loopback only; refusing non-loopback bind".into());
        }
        let listener = TcpListener::bind(addr).map_err(|err| err.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let addr = listener.local_addr().map_err(|err| err.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::spawn(move || serve(listener, sub, flag));
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

fn serve(listener: TcpListener, sub: Subscribe, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_client(stream, &sub);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_client(mut stream: TcpStream, sub: &Subscribe) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    let path = request_path(&head);
    let (status, content_type, body) = match path {
        Some("/" | "/index.html") => ("200 OK", "text/html; charset=utf-8", INDEX_HTML.to_string()),
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
}
