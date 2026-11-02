//! Optional web console surface, std only (no tokio/axum/serde).
//!
//! `present` stores the latest frame as JSON in a single-slot mailbox (latest wins,
//! O(1), never blocks on clients). `open` optionally binds a tiny blocking HTTP server
//! on its own thread that answers:
//!   GET /dot    -> the one dot for the latest frame ([`crate::dot::DotParams::to_json`])
//!   GET /health -> "ok"
//!   GET / , /screen -> the caller's single-screen HTML page, only when one was set with
//!                      [`WebSurface::with_page`] (V1-01; the page itself is owned by
//!                      galion-console, this crate only serves the bytes); 404 otherwise
//! `/dot` also carries `"mode"` (idle|listening|speaking|rika) and `"lean"` (Rika lean-in 0..1).
//! FP-02: the old `/frame` (full VizFrame JSON for a web console that never existed) is gone;
//! `/` + `/dot` + `/health` is the whole surface. `VizFrame::to_json` stays for other callers.
//! **Localhost only.** `serve` refuses any non-loopback address at `open()`
//! (`RenderError::Unsupported`); reach it over an SSH tunnel on the VPS. There is no
//! public-bind switch in this crate by design (FINAL-BUILD-SPEC / A12).
//! The HTML/JS console itself is owned by galion-console and is not embedded here.

use crate::frame::VizFrame;
use crate::surface::{RenderError, RenderSurface, SurfaceCaps, SurfaceKind};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct WebSurface {
    bind: Option<SocketAddr>,
    latest_dot: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
    local_addr: Option<SocketAddr>,
    worker: Option<JoinHandle<()>>,
    page: Option<&'static str>,
}

impl WebSurface {
    /// Mailbox only (no socket) — for embedding into another server or for tests.
    pub fn mailbox() -> Self {
        Self { bind: None, latest_dot: Arc::new(Mutex::new("{}".into())), stop: Arc::new(AtomicBool::new(false)), local_addr: None, worker: None, page: None }
    }
    /// Serve on `addr` (use "127.0.0.1:0" for an ephemeral port).
    pub fn serve(addr: SocketAddr) -> Self {
        let mut s = Self::mailbox();
        s.bind = Some(addr);
        s
    }
    /// Serve `html` at `/` and `/screen` (set before `open()`).
    pub fn with_page(mut self, html: &'static str) -> Self {
        self.page = Some(html);
        self
    }
    pub fn latest_dot_json(&self) -> String { self.latest_dot.lock().map(|g| g.clone()).unwrap_or_default() }
    pub fn local_addr(&self) -> Option<SocketAddr> { self.local_addr }
}

fn handle(mut stream: std::net::TcpStream, latest_dot: &Mutex<String>, page: Option<&'static str>) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split('?').next().unwrap_or("/");
    let (status, ctype, body) = match path {
        "/" | "/screen" if page.is_some() => ("200 OK", "text/html; charset=utf-8", page.unwrap_or_default().to_string()),
        "/dot" => ("200 OK", "application/json", latest_dot.lock().map(|g| g.clone()).unwrap_or_default()),
        "/health" => ("200 OK", "text/plain", "ok".to_string()),
        _ => ("404 Not Found", "text/plain", "not found".to_string()),
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

impl RenderSurface for WebSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Web }
    fn caps(&self) -> SurfaceCaps { SurfaceCaps { color: true, pixels: true, max_fps: 12.5, size: (0, 0) } }
    fn open(&mut self) -> Result<(), RenderError> {
        let Some(addr) = self.bind else { return Ok(()) };
        if self.worker.is_some() { return Ok(()); }
        if !addr.ip().is_loopback() {
            return Err(RenderError::Unsupported("web surface is localhost-only; refusing non-loopback bind (use an SSH tunnel)"));
        }
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        self.local_addr = Some(listener.local_addr()?);
        self.stop.store(false, Ordering::SeqCst);
        let (latest_dot, stop, page) = (self.latest_dot.clone(), self.stop.clone(), self.page);
        self.worker = Some(std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((s, _)) => { let _ = s.set_nonblocking(false); handle(s, &latest_dot, page) }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(20))
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            }
        }));
        Ok(())
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        let mut d = crate::dot::DotParams::from_frame(f).to_json(f.t_ms);
        if d.ends_with('}') {
            d.pop();
            d.push_str(&format!(",\"mode\":\"{}\",\"lean\":{:.3}}}", crate::dot::screen_mode(f), crate::dot::lean_in_of(f)));
        }
        if let Ok(mut g) = self.latest_dot.lock() { *g = d; }
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.worker.take() { let _ = h.join(); }
        Ok(())
    }
}

impl Drop for WebSurface {
    fn drop(&mut self) { let _ = self.close(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_frame_route() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        w.present(&VizFrame::default()).unwrap();
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(b"GET /frame HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 404"), "{out}");
        w.close().unwrap();
    }

    #[test]
    fn refuses_public_bind() {
        let mut w = WebSurface::serve("0.0.0.0:0".parse().unwrap());
        assert!(matches!(w.open(), Err(RenderError::Unsupported(_))));
    }

    fn get(addr: SocketAddr, path: &str) -> String {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn page_only_when_set_and_rika_mode() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        assert!(get(w.local_addr().unwrap(), "/").starts_with("HTTP/1.1 404"));
        w.close().unwrap();
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        for p in ["/", "/screen", "/?kiosk=1"] {
            let r = get(a, p);
            assert!(r.starts_with("HTTP/1.1 200") && r.contains("text/html") && r.contains("<canvas>"), "{r}");
        }
        let mut f = VizFrame::default();
        f.speak.emotion = crate::frame::Emotion { label: "lean-in", intensity: 0.5, valence: 0.0 };
        w.present(&f).unwrap();
        let r = get(a, "/dot");
        assert!(r.contains("\"state\":\"idle\"") && r.contains("\"mode\":\"rika\"") && r.contains("\"lean\":0.500"), "{r}");
        assert!(r.trim_end().ends_with('}'));
        w.close().unwrap();
    }

    #[test]
    fn serves_dot() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        let mut f = VizFrame::default();
        f.speak.active = true;
        w.present(&f).unwrap();
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(b"GET /dot HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.contains("\"state\":\"speaking\""), "{out}");
        w.close().unwrap();
    }
}
