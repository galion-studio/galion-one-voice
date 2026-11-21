// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback HTTP for the page, and the WebSocket on `/v1`.
//! One connection at a time. A late tick does not burst: if the clock
//! falls behind, the next tick is scheduled from now. That is the same
//! rule as a late heartbeat frame.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::{MockEngine, PassiveEngine, ReplyEngine, SessionEngine};
use crate::protocol::{
    decode_client, decode_server, hello_json, text_json, ClientMsg, PcmFrame, ServerMsg, TICK_MS,
    WS_PATH,
};
use crate::ws::{
    self, decode_frame, encode_frame, header, origin_allowed, read_headers, WsConn, WsError,
    OP_BIN, OP_CLOSE, OP_PING, OP_PONG, OP_TEXT,
};

const INDEX_HTML: &str = include_str!("../assets/index.html");
const CSS: &str = include_str!("../assets/samantha.css");
const LOGIC_JS: &str = include_str!("../assets/dot-logic.js");
const PAGE_JS: &str = include_str!("../assets/samantha.js");

const FORBIDDEN: &[u8] =
    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const BAD: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

pub struct Server {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn spawn(addr: &str, mock: bool) -> Result<Self, ServeError> {
        let listener = bind_loopback(addr)?;
        listener.set_nonblocking(true).map_err(ServeError::Io)?;
        let bound = listener.local_addr().map_err(ServeError::Io)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("samantha-dot".into())
            .spawn(move || accept_loop(listener, flag, mock))
            .map_err(ServeError::Io)?;
        Ok(Self {
            addr: bound,
            stop,
            thread: Some(thread),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug)]
pub enum ServeError {
    NotLoopback(String),
    BadAddr(String),
    Io(std::io::Error),
    Ws(WsError),
    Protocol(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::NotLoopback(a) => write!(f, "refusing non-loopback bind {a}"),
            ServeError::BadAddr(a) => write!(f, "bad address {a}"),
            ServeError::Io(e) => write!(f, "{e}"),
            ServeError::Ws(e) => write!(f, "{e}"),
            ServeError::Protocol(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ServeError {}

#[derive(Debug, Clone)]
pub struct Measure {
    pub tick_samples: usize,
    pub tick_min_ms: f64,
    pub tick_median_ms: f64,
    pub tick_max_ms: f64,
    pub http_us: u128,
    pub index_bytes: usize,
    /// Wall time from the `end` control to the preview caption.
    /// This is the mock think window, not a brain latency.
    pub caption_ms: f64,
}

pub fn bind_loopback(addr: &str) -> Result<TcpListener, ServeError> {
    let parsed: SocketAddr = addr
        .parse()
        .map_err(|_| ServeError::BadAddr(addr.to_string()))?;
    if !parsed.ip().is_loopback() {
        return Err(ServeError::NotLoopback(addr.to_string()));
    }
    TcpListener::bind(parsed).map_err(ServeError::Io)
}

/// Bind an ephemeral loopback port, measure the tick clock and one caption,
/// then stop. Used by `samantha-dot check` and the integration test.
pub fn measure() -> Result<Measure, ServeError> {
    let server = Server::spawn("127.0.0.1:0", true)?;
    let addr = server.addr();
    let http = http_index(&addr)?;
    let ticks = tick_intervals(&addr)?;
    let caption_ms = caption_wait(&addr)?;
    drop(server);
    Ok(Measure {
        tick_samples: ticks.len(),
        tick_min_ms: ticks.iter().copied().fold(f64::MAX, f64::min),
        tick_median_ms: median(&ticks),
        tick_max_ms: ticks.iter().copied().fold(0.0, f64::max),
        http_us: http.0,
        index_bytes: http.1,
        caption_ms,
    })
}

fn accept_loop(listener: TcpListener, stop: Arc<AtomicBool>, mock: bool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                if mock {
                    let mut engine = MockEngine::mock();
                    session(stream, &stop, &mut engine);
                } else {
                    let mut engine = PassiveEngine::passive();
                    session(stream, &stop, &mut engine);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn session<E: ReplyEngine>(
    mut stream: TcpStream,
    stop: &AtomicBool,
    engine: &mut SessionEngine<E>,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    let req = match read_headers(&mut stream) {
        Ok(req) => req,
        Err(_) => return,
    };
    let path = req_path(&req);
    if path != WS_PATH {
        let _ = write_http(&mut stream, path);
        return;
    }
    match open_ws(&mut stream, &req) {
        Ok(()) => {}
        Err(WsError::Origin) => {
            let _ = stream.write_all(FORBIDDEN);
            return;
        }
        Err(_) => return,
    }
    let mut buf = Vec::new();
    if !wait_hello(&mut stream, &mut buf, stop) {
        return;
    }
    if !write_msgs(&mut stream, &engine.on_hello()) {
        return;
    }
    let mut next = Instant::now() + Duration::from_millis(u64::from(TICK_MS));
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if Instant::now() >= next {
            if !write_msgs(&mut stream, &engine.on_tick()) {
                return;
            }
            next += Duration::from_millis(u64::from(TICK_MS));
            let late = Duration::from_millis(u64::from(TICK_MS));
            if next + late < Instant::now() {
                next = Instant::now() + late;
            }
        }
        let wait = next
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        let _ = stream.set_read_timeout(Some(wait));
        let mut tmp = [0u8; 8192];
        match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() > 256 * 1024 {
                    return;
                }
                loop {
                    match decode_frame(&buf) {
                        Ok(Some((frame, used))) => {
                            buf.drain(..used);
                            if !on_frame(&mut stream, engine, frame.opcode, &frame.payload) {
                                return;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => return,
                    }
                }
            }
            Err(e) if timed_out(&e) => {}
            Err(_) => return,
        }
    }
}

fn wait_hello(stream: &mut TcpStream, buf: &mut Vec<u8>, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    loop {
        if stop.load(Ordering::Relaxed) || Instant::now() > deadline {
            return false;
        }
        if let Some(text) = pop_text(buf) {
            return matches!(decode_client(&text), Ok(ClientMsg::Hello));
        }
        let mut tmp = [0u8; 8192];
        match stream.read(&mut tmp) {
            Ok(0) => return false,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) if timed_out(&e) => {}
            Err(_) => return false,
        }
    }
}

fn pop_text(buf: &mut Vec<u8>) -> Option<String> {
    match decode_frame(buf) {
        Ok(Some((frame, used))) => {
            buf.drain(..used);
            if frame.opcode == OP_TEXT {
                String::from_utf8(frame.payload).ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn write_msgs(stream: &mut TcpStream, msgs: &[ServerMsg]) -> bool {
    for msg in msgs {
        if write_text(stream, &msg.to_json()).is_err() {
            return false;
        }
    }
    true
}

fn on_frame<E: ReplyEngine>(
    stream: &mut TcpStream,
    engine: &mut SessionEngine<E>,
    opcode: u8,
    payload: &[u8],
) -> bool {
    match opcode {
        OP_CLOSE => false,
        OP_PING => write_raw(stream, &encode_frame(OP_PONG, payload, None)).is_ok(),
        OP_TEXT => {
            let Ok(text) = std::str::from_utf8(payload) else {
                return true;
            };
            let Ok(msg) = decode_client(text) else {
                return true;
            };
            let out = match msg {
                ClientMsg::Hello => Vec::new(),
                ClientMsg::Stop => engine.on_stop(),
                ClientMsg::Ping => vec![ServerMsg::Pong],
                ClientMsg::Text(_) => engine.on_text(),
            };
            write_msgs(stream, &out)
        }
        OP_BIN => {
            if let Ok(frame) = PcmFrame::decode(payload) {
                return write_msgs(stream, &engine.on_pcm(&frame));
            }
            true
        }
        _ => true,
    }
}

fn open_ws(stream: &mut TcpStream, req: &str) -> Result<(), WsError> {
    let lower = req.to_ascii_lowercase();
    if !lower.contains("upgrade: websocket") || header(req, "Sec-WebSocket-Key").is_none() {
        let _ = stream.write_all(BAD);
        return Err(WsError::Handshake);
    }
    let port = stream.local_addr().map(|a| a.port()).unwrap_or(0);
    if let Some(origin) = header(req, "Origin") {
        if !origin_allowed(origin, port) {
            return Err(WsError::Origin);
        }
    }
    let key = header(req, "Sec-WebSocket-Key").ok_or(WsError::Handshake)?;
    let accept = ws::accept_key(key);
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(resp.as_bytes())?;
    Ok(())
}

fn write_text(stream: &mut TcpStream, text: &str) -> Result<(), ()> {
    write_raw(stream, &encode_frame(OP_TEXT, text.as_bytes(), None))
}

fn write_raw(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), ()> {
    stream.write_all(bytes).map_err(|_| ())
}

fn write_http(stream: &mut TcpStream, path: &str) -> std::io::Result<()> {
    let (status, ctype, body): (&str, &str, &[u8]) = match path {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", INDEX_HTML.as_bytes()),
        "/samantha.css" => ("200 OK", "text/css; charset=utf-8", CSS.as_bytes()),
        "/dot-logic.js" => (
            "200 OK",
            "text/javascript; charset=utf-8",
            LOGIC_JS.as_bytes(),
        ),
        "/samantha.js" => (
            "200 OK",
            "text/javascript; charset=utf-8",
            PAGE_JS.as_bytes(),
        ),
        _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found"),
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

fn req_path(req: &str) -> &str {
    let line = req.split("\r\n").next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let _method = parts.next();
    let raw = parts.next().unwrap_or("/");
    raw.split('?').next().unwrap_or("/")
}

fn timed_out(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

fn http_index(addr: &SocketAddr) -> Result<(u128, usize), ServeError> {
    let mut last = None;
    for _ in 0..30 {
        let started = Instant::now();
        match TcpStream::connect_timeout(addr, Duration::from_millis(200)) {
            Ok(mut stream) => {
                let req = "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
                stream.write_all(req.as_bytes()).map_err(ServeError::Io)?;
                let mut buf = Vec::new();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                stream.read_to_end(&mut buf).map_err(ServeError::Io)?;
                let us = started.elapsed().as_micros();
                let text = String::from_utf8_lossy(&buf);
                if !text.contains("200 OK") || !text.contains("Talk to Samantha") {
                    return Err(ServeError::Protocol("index page".into()));
                }
                let bytes = text.split("\r\n\r\n").nth(1).map(|b| b.len()).unwrap_or(0);
                return Ok((us, bytes));
            }
            Err(e) => {
                last = Some(e);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    Err(ServeError::Io(last.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "index")
    })))
}

fn connect_ws(addr: SocketAddr) -> Result<WsConn, ServeError> {
    let mut last = WsError::Handshake;
    for _ in 0..30 {
        match WsConn::connect(addr) {
            Ok(conn) => return Ok(conn),
            Err(e) => {
                last = e;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    Err(ServeError::Ws(last))
}

fn tick_intervals(addr: &SocketAddr) -> Result<Vec<f64>, ServeError> {
    let mut ws = connect_ws(*addr)?;
    ws.send(OP_TEXT, hello_json().as_bytes())
        .map_err(ServeError::Ws)?;
    let welcome = read_event(&mut ws)?;
    if !matches!(welcome, ServerMsg::Welcome) {
        return Err(ServeError::Protocol("missing welcome".into()));
    }
    let mut times = Vec::new();
    let mut stamps = Vec::new();
    while stamps.len() < 10 {
        let ev = read_event(&mut ws)?;
        if let ServerMsg::Mood { t_ms, .. } = ev {
            times.push(Instant::now());
            stamps.push(t_ms);
        }
    }
    for pair in stamps.windows(2) {
        if pair[1] != pair[0].saturating_add(TICK_MS) {
            return Err(ServeError::Protocol(format!(
                "t_ms {} -> {}",
                pair[0], pair[1]
            )));
        }
    }
    let mut gaps = Vec::new();
    for pair in times.windows(2) {
        gaps.push(pair[1].duration_since(pair[0]).as_secs_f64() * 1000.0);
    }
    Ok(gaps)
}

fn caption_wait(addr: &SocketAddr) -> Result<f64, ServeError> {
    let mut ws = connect_ws(*addr)?;
    ws.send(OP_TEXT, hello_json().as_bytes())
        .map_err(ServeError::Ws)?;
    let _ = read_event(&mut ws)?;
    let started = Instant::now();
    ws.send(OP_TEXT, text_json("").as_bytes())
        .map_err(ServeError::Ws)?;
    let deadline = started + Duration::from_secs(3);
    loop {
        if Instant::now() > deadline {
            return Err(ServeError::Protocol("no caption".into()));
        }
        let ev = read_event(&mut ws)?;
        if let ServerMsg::Turn { reply, .. } = ev {
            if reply == "Hi. I'm here." {
                return Ok(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
}

fn read_event(ws: &mut WsConn) -> Result<ServerMsg, ServeError> {
    let frame = ws.next_frame().map_err(ServeError::Ws)?;
    if frame.opcode != OP_TEXT {
        return Err(ServeError::Protocol("expected text".into()));
    }
    let text =
        std::str::from_utf8(&frame.payload).map_err(|_| ServeError::Protocol("utf8".into()))?;
    decode_server(text).map_err(|e| ServeError::Protocol(e.to_string()))
}

fn median(values: &[f64]) -> f64 {
    let mut copy = values.to_vec();
    copy.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if copy.is_empty() {
        return 0.0;
    }
    let mid = copy.len() / 2;
    if copy.len() % 2 == 0 {
        (copy[mid - 1] + copy[mid]) / 2.0
    } else {
        copy[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_a_public_bind() {
        let err = bind_loopback("0.0.0.0:7881").unwrap_err();
        assert!(matches!(err, ServeError::NotLoopback(_)), "{err}");
    }

    #[test]
    fn page_contract() {
        assert!(INDEX_HTML.contains("id=\"talk\""));
        assert!(!INDEX_HTML.contains("id=\"stop\""));
        assert!(!INDEX_HTML.contains("id=\"captions\""));
        assert!(INDEX_HTML.contains("id=\"caption\""));
        assert!(INDEX_HTML.contains("aria-live=\"polite\""));
        assert!(CSS.contains("prefers-reduced-motion"));
        assert!(LOGIC_JS.contains("const TICK_MS = 80;"));
        assert!(LOGIC_JS.contains("const IDLE_RADIUS = 0.100;"));
        assert!(LOGIC_JS.contains("const VIOLET = [124, 58, 237];"));
        assert!(LOGIC_JS.contains("w: 1.8, z: 0.92"));
        assert!(LOGIC_JS.contains("const OFFLINE_RADIUS = 0.060;"));
        assert!(LOGIC_JS.contains("const OFFLINE_BRIGHTNESS = 0.08;"));
        // The call is `getUserMedia(`. The feature check names the same
        // property without calling it.
        assert_eq!(PAGE_JS.matches("getUserMedia(").count(), 1);
        assert_eq!(PAGE_JS.matches("await openMic()").count(), 1);
        assert!(PAGE_JS.contains("function onTalk"));
        let talk = PAGE_JS.split("function onTalk").nth(1).unwrap();
        assert!(talk.contains("await openMic()"));
        assert!(!PAGE_JS.contains("stop.hidden"));
        for asset in [INDEX_HTML, CSS, LOGIC_JS, PAGE_JS] {
            let lower = asset.to_ascii_lowercase();
            assert!(!lower.contains(".wav"));
            assert!(!lower.contains(".mp3"));
            assert!(!lower.contains(".opus"));
            assert!(!lower.contains(".flac"));
        }
    }

    #[test]
    fn foreign_origin_is_forbidden() {
        let server = Server::spawn("127.0.0.1:0", false).unwrap();
        let mut stream = TcpStream::connect(server.addr()).unwrap();
        let req = "GET /v1/samantha HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: https://example.com\r\n\r\n";
        stream.write_all(req.as_bytes()).unwrap();
        let resp = read_headers(&mut stream).unwrap();
        assert!(resp.starts_with("HTTP/1.1 403"), "{resp}");
    }

    #[test]
    fn measured_tick_and_caption() {
        let m = measure().unwrap();
        eprintln!(
            "tick_samples={} tick_min_ms={:.3} tick_median_ms={:.3} tick_max_ms={:.3} http_us={} index_bytes={} caption_ms={:.3}",
            m.tick_samples, m.tick_min_ms, m.tick_median_ms, m.tick_max_ms, m.http_us, m.index_bytes, m.caption_ms
        );
        assert!(m.tick_samples >= 8, "{}", m.tick_samples);
        assert!(
            (40.0..=140.0).contains(&m.tick_median_ms),
            "median {}",
            m.tick_median_ms
        );
        assert!(m.index_bytes > 500, "{}", m.index_bytes);
        assert!(
            (400.0..=1500.0).contains(&m.caption_ms),
            "caption {}",
            m.caption_ms
        );
    }

    #[test]
    fn js_selfcheck() {
        let out = std::process::Command::new("node")
            .arg("assets/selfcheck.js")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("node");
        assert!(
            out.status.success(),
            "stdout {}\nstderr {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
