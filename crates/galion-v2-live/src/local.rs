// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! In-process keep-alive client for a **loopback** OpenAI-compatible server
//! (llama.cpp `llama-server` with Qwen2.5-1.5B on the voice host).
//!
//! It uses no curl child, no temp file, no TLS, and no key. A small pool of
//! HTTP/1.1 keep-alive `TcpStream`s (TCP_NODELAY) stays open between turns,
//! so a turn pays only the request write. Streaming replies arrive as chunked
//! SSE and are decoded here. Dropping the event receiver cancels the turn:
//! the next send fails, the socket is shut down, and llama-server stops
//! generating. Only `127.0.0.1`, `localhost` and `[::1]` URLs are accepted.
use crate::llm::{LlmCfg, LlmEv};
use crate::now_ms;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `http://127.0.0.1:18080/v1/chat/completions` → ("127.0.0.1:18080", "/v1/chat/completions").
/// None unless the host is loopback and the scheme is plain http.
pub fn split_local(url: &str) -> Option<(String, String)> {
    let rest = url.trim().strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "/") };
    let host = if let Some(h) = hostport.strip_prefix('[') { h.split(']').next()? } else { hostport.rsplit_once(':').map(|(h, _)| h).unwrap_or(hostport) };
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") { return None; }
    let hostport = if hostport.contains(':') && !hostport.ends_with(']') { hostport.to_string() } else { format!("{hostport}:80") };
    Some((hostport, path.to_string()))
}

type Conn = BufReader<TcpStream>;
static POOL: OnceLock<Mutex<Vec<(String, Conn, Instant)>>> = OnceLock::new();
fn pool() -> &'static Mutex<Vec<(String, Conn, Instant)>> { POOL.get_or_init(|| Mutex::new(Vec::new())) }

/// llama-server (cpp-httplib) answers `Keep-Alive: timeout=5`, so it closes an
/// idle socket after 5 s. A pooled socket idle longer than this is dropped
/// instead of written to, which avoids a write to a dead socket and an EOF
/// round trip. `GALION_LLM_KEEPALIVE_MS` sets the limit (default 4000).
fn max_idle() -> Duration {
    Duration::from_millis(std::env::var("GALION_LLM_KEEPALIVE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000))
}

fn connect(hostport: &str) -> Result<Conn, String> {
    let s = TcpStream::connect(hostport).map_err(|e| format!("local LLM connect {hostport}: {e}"))?;
    s.set_nodelay(true).ok();
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    Ok(BufReader::new(s))
}

fn take(hostport: &str) -> Option<Conn> {
    let mut p = pool().lock().ok()?;
    let limit = max_idle();
    p.retain(|(_, _, t)| t.elapsed() < limit); // stale: the server has closed it or is about to
    let i = p.iter().position(|(h, _, _)| h == hostport)?;
    Some(p.swap_remove(i).1)
}
fn give_back(hostport: String, c: Conn) { if let Ok(mut p) = pool().lock() { if p.len() < 8 { p.push((hostport, c, Instant::now())); } } }

/// Idle keep-alive connections held for `hostport` (for tests and logs).
pub fn idle(hostport: &str) -> usize { pool().lock().map(|p| p.iter().filter(|(h, _, _)| h == hostport).count()).unwrap_or(0) }

/// Request body for llama-server. Carries no `provider`, `usage` or `reasoning`
/// field. `cache_prompt` keeps the system-prompt KV across turns.
pub fn body(cfg: &LlmCfg, messages: &serde_json::Value) -> String {
    serde_json::json!({"model": cfg.model, "messages": messages, "stream": true, "max_tokens": cfg.max_tokens,
        "temperature": cfg.temperature, "cache_prompt": true}).to_string()
}

struct Head { status: u16, chunked: bool, len: Option<usize>, close: bool }

fn read_head(c: &mut Conn) -> Result<Head, String> {
    let mut line = String::new();
    if c.read_line(&mut line).map_err(|e| e.to_string())? == 0 { return Err("eof before status".into()); }
    let status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or_else(|| format!("bad status line {line:?}"))?;
    let (mut chunked, mut len, mut close) = (false, None, line.starts_with("HTTP/1.0"));
    loop {
        line.clear();
        if c.read_line(&mut line).map_err(|e| e.to_string())? == 0 { return Err("eof in headers".into()); }
        let l = line.trim_end();
        if l.is_empty() { break; }
        if let Some((k, v)) = l.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
            match k.as_str() {
                "transfer-encoding" => chunked = v.contains("chunked"),
                "content-length" => len = v.parse().ok(),
                "connection" => close = v.contains("close"),
                _ => {}
            }
        }
    }
    Ok(Head { status, chunked, len, close })
}

/// Read the body, calling `on` with each piece. `on` returns false to cancel.
/// Returns true when the body ended cleanly (connection reusable).
fn read_body(c: &mut Conn, h: &Head, on: &mut dyn FnMut(&[u8]) -> bool) -> Result<bool, String> {
    if h.chunked {
        let mut line = String::new();
        loop {
            line.clear();
            c.read_line(&mut line).map_err(|e| e.to_string())?;
            let size = usize::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16).map_err(|_| format!("bad chunk size {line:?}"))?;
            if size == 0 {
                loop { line.clear(); if c.read_line(&mut line).map_err(|e| e.to_string())? == 0 || line.trim_end().is_empty() { break; } }
                return Ok(true);
            }
            let mut buf = vec![0u8; size];
            c.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let mut crlf = [0u8; 2];
            c.read_exact(&mut crlf).map_err(|e| e.to_string())?;
            if !on(&buf) { return Ok(false); }
        }
    } else if let Some(n) = h.len {
        let mut buf = vec![0u8; n];
        c.read_exact(&mut buf).map_err(|e| e.to_string())?;
        on(&buf);
        Ok(true)
    } else {
        let mut buf = Vec::new();
        c.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        on(&buf);
        Ok(false)
    }
}

fn send(c: &mut Conn, hostport: &str, path: &str, body: &str) -> std::io::Result<()> {
    let s = c.get_mut();
    let head = format!("POST {path} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\nAccept: text/event-stream\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n", body.len());
    let mut req = Vec::with_capacity(head.len() + body.len());
    req.extend_from_slice(head.as_bytes()); req.extend_from_slice(body.as_bytes());
    s.write_all(&req)?; s.flush()
}

/// Send `body`, reusing an idle keep-alive connection when there is one.
/// A stale pooled socket (server closed it) is retried once on a fresh one.
fn open(hostport: &str, path: &str, body: &str) -> Result<(Conn, Head, bool), String> {
    if let Some(mut c) = take(hostport) {
        if send(&mut c, hostport, path, body).is_ok() {
            if let Ok(h) = read_head(&mut c) { return Ok((c, h, true)); }
        }
    }
    let mut c = connect(hostport)?;
    send(&mut c, hostport, path, body).map_err(|e| format!("local LLM write: {e}"))?;
    let h = read_head(&mut c)?;
    Ok((c, h, false))
}

/// Streaming chat completion against the loopback server. Same events as
/// `llm::stream`. `Done` carries no cost and a provider tag: `local` (new
/// socket) or `local-reused` (kept-alive socket), followed by llama-server's
/// own timings when it sends them, e.g.
/// `local-reused cache_n=96 prompt_n=21 prompt_ms=88.0 tok_s=24.1`.
pub fn stream(cfg: &LlmCfg, messages: serde_json::Value) -> Result<Receiver<LlmEv>, String> {
    let (hostport, path) = split_local(&cfg.url).ok_or("GALION_LLM_URL is not a loopback http URL")?;
    let body = body(cfg, &messages);
    let (mut c, head, reused) = open(&hostport, &path, &body)?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let tag = if reused { "local-reused" } else { "local" };
        if head.status != 200 {
            let mut err = Vec::new();
            let _ = read_body(&mut c, &head, &mut |b| { err.extend_from_slice(b); true });
            eprintln!("local LLM HTTP {}: {}", head.status, String::from_utf8_lossy(&err[..err.len().min(300)]));
            let _ = tx.send(LlmEv::Done(now_ms(), None, format!("{tag}-http-{}", head.status)));
            return;
        }
        let mut pending: Vec<u8> = Vec::new();
        let mut done = false;
        let mut timings = String::new();
        let mut on = |b: &[u8]| -> bool {
            pending.extend_from_slice(b);
            while let Some(nl) = pending.iter().position(|&x| x == b'\n') {
                let line: Vec<u8> = pending.drain(..=nl).collect();
                let line = String::from_utf8_lossy(&line);
                let Some(d) = line.trim_end().strip_prefix("data: ") else { continue };
                if d.trim() == "[DONE]" { done = true; continue; }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(d) else { continue };
                let tm = &v["timings"];
                if tm.is_object() {
                    timings = format!(" cache_n={} prompt_n={} prompt_ms={:.1} tok_s={:.1}", tm["cache_n"], tm["prompt_n"],
                        tm["prompt_ms"].as_f64().unwrap_or(-1.0), tm["predicted_per_second"].as_f64().unwrap_or(-1.0));
                }
                let delta = &v["choices"][0]["delta"];
                let t = now_ms();
                if let Some(r) = delta["reasoning_content"].as_str() { if !r.is_empty() && tx.send(LlmEv::Thought(t, r.to_string())).is_err() { return false; } }
                if let Some(s) = delta["content"].as_str() { if !s.is_empty() && tx.send(LlmEv::Token(t, s.to_string())).is_err() { return false; } }
            }
            true
        };
        match read_body(&mut c, &head, &mut on) {
            Ok(true) if !head.close => give_back(hostport, c),
            Ok(false) => { let _ = c.get_ref().shutdown(Shutdown::Both); } // cancelled: the server stops generating
            Ok(true) => {}
            Err(e) => eprintln!("local LLM read: {e}"),
        }
        let _ = done;
        let _ = tx.send(LlmEv::Done(now_ms(), None, format!("{tag}{timings}")));
    });
    Ok(rx)
}

/// Pre-turn warm-up: opens the keep-alive socket and fills llama-server's
/// prompt cache with `system` (one 1-token completion). Returns wall ms.
pub fn prewarm(cfg: &LlmCfg, system: &str) -> Result<f64, String> {
    let t0 = now_ms();
    let mut c1 = LlmCfg { max_tokens: 1, ..cfg.clone() };
    c1.temperature = 0.0;
    let rx = stream(&c1, serde_json::json!([{"role": "system", "content": system}, {"role": "user", "content": "Hi."}]))?;
    for ev in rx.iter() { if let LlmEv::Done(_, _, p) = ev { if p.contains("http-") { return Err(format!("prewarm failed: {p}")); } break; } }
    Ok(now_ms() - t0)
}

#[cfg(test)]
mod t {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn only_loopback_urls_are_local() {
        assert_eq!(split_local("http://127.0.0.1:18080/v1/chat/completions"), Some(("127.0.0.1:18080".into(), "/v1/chat/completions".into())));
        assert_eq!(split_local("http://localhost:8080/x").unwrap().0, "localhost:8080");
        assert_eq!(split_local("http://[::1]:9/v1").unwrap().0, "[::1]:9");
        assert!(split_local("https://openrouter.ai/api/v1/chat/completions").is_none());
        assert!(split_local("http://10.0.0.5:8080/v1").is_none());
        assert!(split_local("http://127.0.0.1.evil.com/v1").is_none());
    }

    #[test]
    fn local_body_has_no_provider_or_usage() {
        let cfg = LlmCfg { url: "http://127.0.0.1:1/v1".into(), model: "q".into(), max_tokens: 9, reasoning: None, provider_order: vec![], temperature: 0.7 };
        let v: serde_json::Value = serde_json::from_str(&body(&cfg, &serde_json::json!([]))).unwrap();
        assert!(v.get("provider").is_none() && v.get("usage").is_none() && v.get("reasoning").is_none());
        assert_eq!(v["cache_prompt"], true);
        assert_eq!(v["stream"], true);
    }

    /// A chunked SSE server that answers two requests on ONE connection: the
    /// second turn must reuse the kept-alive socket.
    #[test]
    fn chunked_sse_and_keep_alive_reuse() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let srv = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut r = BufReader::new(s);
            for reply in ["Hi", "Again"] {
                let mut len = 0usize; let mut line = String::new();
                loop { line.clear(); r.read_line(&mut line).unwrap(); if line == "\r\n" { break; }
                       if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") { len = v.trim().parse().unwrap(); } }
                let mut b = vec![0u8; len]; r.read_exact(&mut b).unwrap();
                let w = r.get_mut();
                write!(w, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nKeep-Alive: timeout=5\r\n\r\n").unwrap();
                for ev in [format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{reply}\"}}}}]}}\n\n"), " there".into(), "data: [DONE]\n\n".into()] {
                    // split one event across chunks on purpose
                    let ev = if ev == " there" { "data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}\n\n".to_string() } else { ev };
                    let (a, b2) = ev.split_at(ev.len() / 2);
                    for part in [a, b2] { write!(w, "{:x}\r\n{part}\r\n", part.len()).unwrap(); }
                }
                write!(w, "0\r\n\r\n").unwrap();
            }
        });
        let cfg = LlmCfg { url: format!("http://127.0.0.1:{port}/v1/chat/completions"), model: "q".into(), max_tokens: 9, reasoning: None, provider_order: vec![], temperature: 0.7 };
        let mut tags = vec![];
        for want in ["Hi there", "Again there"] {
            let rx = stream(&cfg, serde_json::json!([{"role":"user","content":"x"}])).unwrap();
            let mut text = String::new();
            for ev in rx.iter() { match ev { LlmEv::Token(_, s) => text.push_str(&s), LlmEv::Done(_, _, p) => { tags.push(p.split(' ').next().unwrap().to_string()); break; } _ => {} } }
            assert_eq!(text, want);
        }
        assert_eq!(tags, vec!["local".to_string(), "local-reused".to_string()]);
        srv.join().unwrap();
    }
}
